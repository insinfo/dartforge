//! Inlining of trivial functions (`docs/JS-PRODUCAO-TAMANHO.md` §10.3).
//!
//! The SDK is full of one-expression functions: the accessors of `dart:_rti`
//! (`static _getKind(rti) { return rti[_kind]; }`), the identity casts of
//! `_Utils` (`static asRti(s) { return s; }`), `_recipeJoin(s1, s2)`... Each
//! call costs the qualified name of the function (`R._getKind(…)`), which
//! `dart2js` never pays: its SSA inlines them
//! (`ssa/builder.dart:7851-8075`). Here the call `C.m(a, b)` (a static method
//! of a class assigned as `C = class …`) or `f(a, b)` (a function assigned
//! once as `f = function(…) {…}` or declared with `function f`) becomes the
//! returned expression with the arguments in place of the parameters.
//!
//! The rule keeps JavaScript's evaluation order: the arguments are evaluated
//! before the body. The body must be a single `return E;`, where `E` has no
//! function, `this`, assignment or update, and uses each parameter at most
//! once. When `E` evaluates anything other than a parameter (a call, a
//! property read, a free name) before its last parameter, or uses the
//! parameters out of order, or under a condition, the inlining only happens
//! at calls whose arguments are stable primitive literals or `this`. Names
//! can be changed by a callback in the body, and property reads can invoke
//! getters or throw, so neither can move across body evaluation without proof. A
//! parameter the body never reads may have a default value (`n = null`, an
//! optional positional parameter) and may be left out by the call; when the
//! call passes it, the argument must have no effect at all (a literal or `this`), because it disappears: `checkBinding(old, new, "expr", loc)` of
//! the ngdart views, which only compares the first two in production, becomes
//! the comparison.
//!
//! The free names of `E` must mean the same at the call: a name declared
//! inside any function other than the top-level ones (or by a class) is
//! refused, and a name declared at the top of a module's IIFE is only valid
//! inside that IIFE.
//!
//! The replacement is textual, through `oxc`'s spans, in a few rounds (a body
//! that calls another trivial function is inlined in the next round). A
//! definition whose name no longer appears anywhere else is removed.
//! `DARTFORGE_JSPROD_INLINE=0` turns it off.

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Class, ClassElement, Expression, FormalParameters, Function, FunctionBody,
    MethodDefinitionKind, ObjectPropertyKind, PropertyKey, Statement, UnaryOperator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;
use std::collections::{HashMap, HashSet};

/// The longest body (in source bytes) that is inlined.
const MAX_BODY: usize = 400;

/// A trivial function: `key` is the callee text (`C.m` or `f`).
#[derive(Debug, Clone)]
struct Def {
    /// Span of the returned expression.
    expr: (u32, u32),
    /// Number of parameters.
    arity: usize,
    /// Which parameters the body uses. A call may leave out trailing unused
    /// parameters, and the argument of an unused one must be [`droppable`].
    used: Vec<bool>,
    /// Spans of the parameter uses inside `expr`, with the parameter index.
    uses: Vec<(u32, u32, usize)>,
    /// Free names of `expr`.
    free: Vec<String>,
    /// The inlining needs simple arguments (see the module documentation).
    simple_args: bool,
    /// The top-level IIFE (depth 2) of the definition.
    iife: Option<usize>,
    /// The function's own span: calls inside it are not inlined.
    own: (u32, u32),
    /// What to delete when the name is no longer cited, and the name
    /// (empty: never removed).
    removal: (u32, u32, String),
    /// Defined by `f = function…` (one assignment is its own).
    from_assignment: bool,
    /// How many citations of the name the definition itself accounts for.
    allowed: usize,
    /// [`minified_estimate`] of the body without its parameters, of the
    /// callee with the parentheses and commas, and of the definition.
    body_size: usize,
    call_size: usize,
    def_size: usize,
}

/// A rough size of `text` after minification: names and property names
/// shortened (2 and 3 bytes), literals as they are, punctuation 1 byte.
fn minified_estimate(text: &str) -> usize {
    let b = text.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            let s = i;
            i += 1;
            while i < b.len() && b[i] != c {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
            n += i.min(b.len()) - s;
        } else if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && (id(b[i]) || b[i] == b'.') {
                i += 1;
            }
            n += i - s;
        } else if id(c) {
            let s = i;
            while i < b.len() && id(b[i]) {
                i += 1;
            }
            let property = s > 0 && b[s - 1] == b'.';
            n += (i - s).min(if property { 3 } else { 2 });
        } else {
            if !c.is_ascii_whitespace() {
                n += 1;
            }
            i += 1;
        }
    }
    n
}

/// What the analysis of the returned expression gathers.
struct Body<'p> {
    params: &'p [String],
    uses: Vec<(u32, u32, usize)>,
    free: Vec<String>,
    /// Something other than a parameter was evaluated.
    evaluated: bool,
    /// Under `?:`, `&&`, `||`, `??`.
    conditional: usize,
    simple_args: bool,
    last_param: Option<usize>,
}

impl Body<'_> {
    /// `false`: the expression cannot be inlined.
    fn expr(&mut self, e: &Expression<'_>) -> bool {
        match e {
            Expression::Identifier(i) => {
                if let Some(k) = self.params.iter().position(|p| p == i.name.as_str()) {
                    if self.uses.iter().any(|u| u.2 == k) {
                        return false;
                    }
                    if self.evaluated || self.conditional > 0 || self.last_param.is_some_and(|l| l > k) {
                        self.simple_args = true;
                    }
                    self.last_param = Some(k);
                    self.uses.push((i.span.start, i.span.end, k));
                } else {
                    self.free.push(i.name.to_string());
                    self.evaluated = true;
                }
                true
            }
            Expression::StringLiteral(_) | Expression::NumericLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_) => true,
            Expression::StaticMemberExpression(m) => {
                if m.optional || !self.expr(&m.object) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::ComputedMemberExpression(m) => {
                if m.optional || !self.expr(&m.object) || !self.expr(&m.expression) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::CallExpression(c) => {
                if c.optional || !self.expr(&c.callee) || !self.arguments(&c.arguments) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::NewExpression(c) => {
                if !self.expr(&c.callee) || !self.arguments(&c.arguments) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::BinaryExpression(b) => {
                if !self.expr(&b.left) || !self.expr(&b.right) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::LogicalExpression(l) => {
                if !self.expr(&l.left) {
                    return false;
                }
                self.conditional += 1;
                let ok = self.expr(&l.right);
                self.conditional -= 1;
                self.evaluated = true;
                ok
            }
            Expression::ConditionalExpression(c) => {
                if !self.expr(&c.test) {
                    return false;
                }
                self.conditional += 1;
                let ok = self.expr(&c.consequent) && self.expr(&c.alternate);
                self.conditional -= 1;
                self.evaluated = true;
                ok
            }
            Expression::UnaryExpression(u) => {
                if u.operator == UnaryOperator::Delete || !self.expr(&u.argument) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::ParenthesizedExpression(p) => self.expr(&p.expression),
            Expression::ArrayExpression(a) => a.elements.iter().all(|x| match x {
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => false,
                x => x.as_expression().is_some_and(|x| self.expr(x)),
            }),
            Expression::ObjectExpression(o) => o.properties.iter().all(|p| match p {
                ObjectPropertyKind::ObjectProperty(p) => !p.computed && !p.method && !p.shorthand && self.expr(&p.value),
                ObjectPropertyKind::SpreadProperty(_) => false,
            }),
            _ => false,
        }
    }

    fn arguments(&mut self, args: &[Argument<'_>]) -> bool {
        args.iter().all(|a| a.as_expression().is_some_and(|x| self.expr(x)))
    }
}

/// An argument whose value and effects cannot change when body evaluation
/// moves before it. Mutable names and property reads require a separate proof.
fn stable_argument(e: &Expression<'_>) -> bool {
    match e {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::ThisExpression(_) => true,
        Expression::ParenthesizedExpression(p) => stable_argument(&p.expression),
        _ => false,
    }
}

/// The parameter names, if they are all plain identifiers, with whether each
/// has a default value (`n = null`, the optional positional parameters).
fn plain_params(p: &FormalParameters<'_>) -> Option<Vec<(String, bool)>> {
    if p.rest.is_some() {
        return None;
    }
    p.items
        .iter()
        .map(|x| match &x.pattern {
            BindingPattern::BindingIdentifier(b) => Some((b.name.to_string(), x.initializer.is_some())),
            _ => None,
        })
        .collect()
}

/// An unused argument can disappear only when evaluation cannot throw or
/// invoke an accessor. A bare name needs a separate initialization proof.
fn droppable(e: &Expression<'_>) -> bool {
    stable_argument(e)
}

/// The returned expression of a body that is a single `return E;`.
fn single_return<'b, 'a>(body: &'b FunctionBody<'a>) -> Option<&'b Expression<'a>> {
    if !body.directives.is_empty() || body.statements.len() != 1 {
        return None;
    }
    match &body.statements[0] {
        Statement::ReturnStatement(r) => r.argument.as_ref(),
        _ => None,
    }
}

/// The first pass: declarations, definitions, assignments.
struct Scan<'s> {
    source: &'s str,
    /// Function nesting (the wrapper of the whole file is depth 1).
    depth: usize,
    /// Index of the current depth-2 function (a module's IIFE).
    iife: Option<usize>,
    iifes: usize,
    /// Spans of the statements directly in the body of a depth-1 or depth-2
    /// function, with that depth.
    top_statements: HashSet<u32>,
    /// Names declared at depth 1 (`None`) or at the top of an IIFE.
    top_names: HashMap<String, HashSet<Option<usize>>>,
    /// Names declared anywhere else (locals, parameters, class names).
    locals: HashSet<String>,
    /// Assignments per target text (`C.m`, `f`).
    assigned: HashMap<String, usize>,
    defs: HashMap<String, Def>,
    /// Keys defined more than once: never inlined.
    refused: HashSet<String>,
    /// Spans of the top-level binding identifiers (not locals).
    top_bindings: HashSet<u32>,
    /// Top-level `var` declarators per name.
    top_decls: HashMap<String, usize>,
}

impl<'s> Scan<'s> {
    fn text(&self, s: oxc_span::Span) -> &'s str {
        &self.source[s.start as usize..s.end as usize]
    }

    fn define(&mut self, key: String, params: Option<Vec<(String, bool)>>, body: Option<&FunctionBody<'_>>, own: (u32, u32), removal: (u32, u32, String), from_assignment: bool) {
        if self.defs.contains_key(&key) || self.refused.contains(&key) {
            self.defs.remove(&key);
            self.refused.insert(key);
            return;
        }
        let (Some(params), Some(body)) = (params, body) else {
            self.refused.insert(key);
            return;
        };
        let Some(e) = single_return(body) else {
            self.refused.insert(key);
            return;
        };
        let span = e.span();
        if (span.end - span.start) as usize > MAX_BODY {
            self.refused.insert(key);
            return;
        }
        let (params, defaults): (Vec<String>, Vec<bool>) = params.into_iter().unzip();
        let mut b = Body { params: &params, uses: Vec::new(), free: Vec::new(), evaluated: false, conditional: 0, simple_args: false, last_param: None };
        // `arguments` would be the caller's after the inlining.
        if !b.expr(e) || b.free.iter().any(|n| n == "arguments") {
            self.refused.insert(key);
            return;
        }
        let used: Vec<bool> = (0..params.len()).map(|k| b.uses.iter().any(|u| u.2 == k)).collect();
        // A default value only matters when the parameter is read.
        if used.iter().zip(&defaults).any(|(&u, &d)| u && d) {
            self.refused.insert(key);
            return;
        }
        let body_size = minified_estimate(&self.source[span.start as usize..span.end as usize]).saturating_sub(2 * b.uses.len());
        let call_size = minified_estimate(&key) + 2 + params.len().saturating_sub(1);
        let def = Def {
            expr: (span.start, span.end),
            arity: params.len(),
            used,
            uses: b.uses,
            free: b.free,
            simple_args: b.simple_args,
            iife: self.iife,
            own,
            removal,
            from_assignment,
            allowed: 1,
            body_size,
            call_size,
            def_size: minified_estimate(&self.source[own.0 as usize..own.1 as usize]),
        };
        self.defs.insert(key, def);
    }

    fn class_statics(&mut self, owner: &str, class: &Class<'_>) {
        for el in class.body.body.iter() {
            let ClassElement::MethodDefinition(m) = el else { continue };
            if !m.r#static || m.computed || m.kind != MethodDefinitionKind::Method {
                continue;
            }
            let PropertyKey::StaticIdentifier(k) = &m.key else { continue };
            let f = &m.value;
            if f.generator || f.r#async {
                continue;
            }
            let key = format!("{owner}.{}", k.name);
            self.define(key, plain_params(&f.params), f.body.as_deref(), (m.span.start, m.span.end), (m.span.start, m.span.end, k.name.to_string()), false);
        }
    }

    fn function_value(&mut self, name: &str, f: &Function<'_>, own: (u32, u32), removal: (u32, u32, String), from_assignment: bool) {
        if f.generator || f.r#async {
            self.refused.insert(name.to_string());
            return;
        }
        self.define(name.to_string(), plain_params(&f.params), f.body.as_deref(), own, removal, from_assignment);
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        let saved = self.iife;
        if self.depth == 2 {
            self.iife = Some(self.iifes);
            self.iifes += 1;
        }
        if self.depth <= 2 {
            if let Some(b) = &it.body {
                for s in b.statements.iter() {
                    self.top_statements.insert(s.span().start);
                }
            }
        }
        walk::walk_function(self, it, flags);
        self.iife = saved;
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.depth += 1;
        walk::walk_arrow_function_expression(self, it);
        self.depth -= 1;
    }
    fn visit_statement(&mut self, it: &Statement<'a>) {
        if self.top_statements.contains(&it.span().start) {
            let scope = if self.depth == 1 { None } else { self.iife };
            match it {
                Statement::VariableDeclaration(d) => {
                    for v in d.declarations.iter() {
                        let BindingPattern::BindingIdentifier(b) = &v.id else { continue };
                        let name = b.name.to_string();
                        self.top_names.entry(name.clone()).or_default().insert(scope);
                        self.top_bindings.insert(b.span.start);
                        *self.top_decls.entry(name.clone()).or_default() += 1;
                        match &v.init {
                            Some(Expression::ClassExpression(c)) => self.class_statics(&name, c),
                            // A `var` declarator is never removed (the statement may hold others).
                            Some(Expression::FunctionExpression(f)) => self.function_value(&name, f, (f.span.start, f.span.end), (0, 0, String::new()), false),
                            _ => {}
                        }
                    }
                }
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        let name = id.name.to_string();
                        self.top_names.entry(name.clone()).or_default().insert(scope);
                        self.top_bindings.insert(id.span.start);
                        self.function_value(&name, f, (f.span.start, f.span.end), (f.span.start, f.span.end, name.clone()), false);
                    }
                }
                Statement::ExpressionStatement(s) => {
                    if let Expression::AssignmentExpression(a) = &s.expression {
                        let target = match &a.left {
                            AssignmentTarget::AssignmentTargetIdentifier(i) => Some(i.name.to_string()),
                            AssignmentTarget::StaticMemberExpression(m) => Some(self.text(m.span).to_string()),
                            _ => None,
                        };
                        if let Some(t) = target {
                            match &a.right {
                                Expression::FunctionExpression(f) => {
                                    self.function_value(&t, f, (f.span.start, f.span.end), (s.span.start, s.span.end, t.clone()), true)
                                }
                                Expression::ClassExpression(c) => self.class_statics(&t, c),
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        walk::walk_statement(self, it);
    }
    fn visit_binding_identifier(&mut self, it: &oxc_ast::ast::BindingIdentifier<'a>) {
        if !self.top_bindings.contains(&it.span.start) {
            self.locals.insert(it.name.to_string());
        }
    }
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        let target = match &it.left {
            AssignmentTarget::AssignmentTargetIdentifier(i) => Some(i.name.to_string()),
            AssignmentTarget::StaticMemberExpression(m) => Some(self.text(m.span).to_string()),
            _ => None,
        };
        if let Some(t) = target {
            *self.assigned.entry(t).or_default() += 1;
        }
        walk::walk_assignment_expression(self, it);
    }
    fn visit_update_expression(&mut self, it: &oxc_ast::ast::UpdateExpression<'a>) {
        let t = self.text(it.argument.span()).to_string();
        *self.assigned.entry(t).or_default() += 2;
        walk::walk_update_expression(self, it);
    }
}

/// The second pass: the calls to inline.
struct Sites<'s> {
    source: &'s str,
    defs: &'s HashMap<String, Def>,
    depth: usize,
    iife: Option<usize>,
    iifes: usize,
    /// (start, end, new text), not overlapping.
    edits: Vec<(u32, u32, String)>,
    /// End of the last edit (calls inside it wait for the next round).
    covered: u32,
}

impl<'a> Visit<'a> for Sites<'_> {
    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        self.depth += 1;
        let saved = self.iife;
        if self.depth == 2 {
            self.iife = Some(self.iifes);
            self.iifes += 1;
        }
        walk::walk_function(self, it, flags);
        self.iife = saved;
        self.depth -= 1;
    }
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if it.span.start >= self.covered {
            if let Some(new) = self.inline(it) {
                self.edits.push((it.span.start, it.span.end, new));
                self.covered = it.span.end;
                return;
            }
        }
        walk::walk_call_expression(self, it);
    }
}

impl Sites<'_> {
    fn inline(&self, it: &oxc_ast::ast::CallExpression<'_>) -> Option<String> {
        if it.optional {
            return None;
        }
        let key = match &it.callee {
            Expression::Identifier(i) => i.name.as_str(),
            Expression::StaticMemberExpression(m) if !m.optional && matches!(m.object, Expression::Identifier(_)) => {
                &self.source[m.span.start as usize..m.span.end as usize]
            }
            _ => return None,
        };
        let d = self.defs.get(key)?;
        // Fewer arguments: the parameters left out must be unused.
        if it.arguments.len() > d.arity || d.used[it.arguments.len()..].iter().any(|&u| u) || (it.span.start >= d.own.0 && it.span.end <= d.own.1) {
            return None;
        }
        // Free names declared at the top of an IIFE only mean the same in it.
        if d.iife.is_some() && d.iife != self.iife {
            return None;
        }
        let args: Vec<&Expression<'_>> = it.arguments.iter().map(|a| a.as_expression()).collect::<Option<_>>()?;
        if d.simple_args && !args.iter().all(|a| stable_argument(a)) {
            return None;
        }
        // The arguments of unused parameters are dropped.
        if args.iter().zip(&d.used).any(|(a, &u)| !u && !droppable(a)) {
            return None;
        }
        let mut out = String::from("(");
        let mut pos = d.expr.0;
        for &(s, e, k) in &d.uses {
            out.push_str(&self.source[pos as usize..s as usize]);
            let a = args[k].span();
            out.push('(');
            out.push_str(&self.source[a.start as usize..a.end as usize]);
            out.push(')');
            pos = e;
        }
        out.push_str(&self.source[pos as usize..d.expr.1 as usize]);
        out.push(')');
        Some(out)
    }
}

/// Applies `edits` (sorted, not overlapping) to `source`.
fn rewrite(source: &str, mut edits: Vec<(u32, u32, String)>) -> String {
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(source.len());
    let mut pos = 0usize;
    for (s, e, new) in edits {
        if (s as usize) < pos {
            continue;
        }
        out.push_str(&source[pos..s as usize]);
        out.push_str(&new);
        pos = e as usize;
    }
    out.push_str(&source[pos..]);
    out
}

/// One round: the trivial functions and their calls. Returns the definitions
/// too, for the final removal.
fn round(source: &str) -> Result<(String, usize, HashMap<String, Def>), String> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let mut scan = Scan {
        source,
        depth: 0,
        iife: None,
        iifes: 0,
        top_statements: HashSet::new(),
        top_names: HashMap::new(),
        locals: HashSet::new(),
        assigned: HashMap::new(),
        defs: HashMap::new(),
        refused: HashSet::new(),
        top_bindings: HashSet::new(),
        top_decls: HashMap::new(),
    };
    scan.visit_program(&parsed.program);
    let mut defs = std::mem::take(&mut scan.defs);
    defs.retain(|key, d| {
        // Assigned again somewhere (`C.m = …`, a second `f = …`).
        if scan.assigned.get(key).copied().unwrap_or(0) > usize::from(d.from_assignment) {
            return false;
        }
        d.free.iter().all(|n| !scan.locals.contains(n) && scan.top_names.get(n).is_none_or(|s| s.len() == 1 && (s.contains(&None) || s.contains(&d.iife))))
    });
    for (key, d) in defs.iter_mut() {
        // A definition whose free names are all global or of the file's top
        // can be inlined anywhere.
        if d.free.iter().all(|n| scan.top_names.get(n).is_none_or(|s| s.contains(&None))) {
            d.iife = None;
        }
        // `f = function…` with `var f` in the hoisted list.
        if d.from_assignment {
            d.allowed = 1 + scan.top_decls.get(key).copied().unwrap_or(0);
        }
    }
    // Inlined only where the file gets shorter: each call trades the callee
    // for the body, and the definition goes when the calls were all of its
    // citations.
    let counts = identifier_counts(source);
    defs.retain(|key, d| {
        let name = key.rsplit('.').next().unwrap_or(key);
        let calls = counts.get(name).copied().unwrap_or(0).saturating_sub(d.allowed);
        let removed = if d.removal.2.is_empty() { 0 } else { d.def_size };
        calls * d.call_size + removed >= calls * d.body_size
    });
    let mut sites = Sites { source, defs: &defs, depth: 0, iife: None, iifes: 0, edits: Vec::new(), covered: 0 };
    sites.visit_program(&parsed.program);
    let n = sites.edits.len();
    let edits = std::mem::take(&mut sites.edits);
    Ok((rewrite(source, edits), n, defs))
}

/// How many times each identifier-shaped word occurs in `text`.
fn identifier_counts(text: &str) -> HashMap<&str, usize> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    let b = text.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut i = 0;
    while i < b.len() {
        if id(b[i]) {
            let s = i;
            while i < b.len() && id(b[i]) {
                i += 1;
            }
            *count.entry(&text[s..i]).or_default() += 1;
        } else {
            i += 1;
        }
    }
    count
}

/// Counts the identifier-shaped occurrences of `name` in `text`.
fn occurrences(text: &str, name: &str) -> usize {
    let b = text.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    text.match_indices(name)
        .filter(|(p, _)| {
            let before = *p == 0 || !id(b[p - 1]);
            let end = p + name.len();
            before && (end >= b.len() || !id(b[end]))
        })
        .count()
}

/// Inlines the trivial functions of `source` (the whole file, already
/// flattened) and removes the definitions left without uses.
///
/// # Errors
///
/// When `oxc` cannot parse the text.
pub fn apply(source: &str) -> Result<String, String> {
    let mut text = source.to_string();
    let mut settled: Option<HashMap<String, Def>> = None;
    for _ in 0..6 {
        let (next, n, defs) = round(&text)?;
        if n == 0 {
            // Nothing changed: the spans of `defs` are those of `text`.
            settled = Some(defs);
            break;
        }
        text = next;
    }
    let Some(defs) = settled else { return Ok(text) };
    let mut removals: Vec<(u32, u32, String)> = Vec::new();
    for d in defs.values() {
        let name = &d.removal.2;
        if !name.is_empty() && occurrences(&text, name) <= d.allowed {
            removals.push((d.removal.0, d.removal.1, String::new()));
        }
    }
    Ok(rewrite(&text, removals))
}

#[cfg(test)]
mod tests {
    use super::apply;

    fn wrap(s: &str) -> String {
        format!("(function () {{\nvar C, f, g;\n(function () {{\n{s}\n}})();\n}})();\n")
    }

    #[test]
    fn inlines_a_static_accessor() {
        let src = wrap("C = class C0 { static k(r) { return r.kind; } };\nconsole.log(C.k(x));");
        let out = apply(&src).unwrap();
        assert!(out.contains("console.log(((x).kind))"), "{out}");
    }

    #[test]
    fn keeps_order_with_effects() {
        // A body evaluation before the parameter requires stable arguments.
        let src = wrap("f = function(a) { return g + a; };\nh(f(y()));\nh(f(z));\nh(f(3));");
        let out = apply(&src).unwrap();
        assert!(out.contains("h(f(y()))"), "{out}");
        assert!(out.contains("h(f(z))"), "{out}");
        assert!(out.contains("h((g + (3)))"), "{out}");
    }

    #[test]
    fn refuses_a_shadowed_free_name() {
        let src = wrap("f = function(a) { return a + v; };\nfunction q(v) { return f(1); }\nq(2);");
        let out = apply(&src).unwrap();
        assert!(out.contains("f(1)") && !out.contains("(1) + v"), "{out}");
    }

    #[test]
    fn refuses_a_reassigned_function() {
        let src = wrap("f = function(a) { return a; };\nf = function(a) { return 2; };\nf(1);");
        let out = apply(&src).unwrap();
        assert!(out.contains("f(1);"), "{out}");
    }

    #[test]
    fn drops_the_arguments_of_unused_parameters() {
        let src = wrap("f = function(a, b, c = null, d = null) { return !g(a, b); };\nh(f(x.y, z, \"expr\", loc));\nh(f(x, z));\nh(f(x, z, w.v));\nh(f(1, 2, \"expr\", 5));");
        let out = apply(&src).unwrap();
        // The callee is looked up in the body; mutable arguments stay eager.
        assert!(out.contains("h(f(x.y, z, \"expr\", loc))"), "{out}");
        assert!(out.contains("h(f(x, z))"), "{out}");
        assert!(out.contains("h((!g((1), (2))))"), "{out}");
        // A property read can throw: its argument is not dropped.
        assert!(out.contains("h(f(x, z, w.v))"), "{out}");
    }

    #[test]
    #[ignore = "requires Node on PATH"]
    fn unused_argument_preserves_temporal_dead_zone_error() {
        let source = wrap("f = function(unused) { return 7; };\ntry { console.log(f(value)); } catch (error) { console.log(error.name); }\nlet value = 1;");
        let transformed = apply(&source).unwrap();
        for code in [&source, &transformed] {
            let output = std::process::Command::new("node").args(["-e", code]).output().unwrap();
            assert!(output.status.success() && output.stderr.is_empty());
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "ReferenceError\n");
        }
    }

    #[test]
    fn refuses_a_used_parameter_with_a_default() {
        let src = wrap("f = function(a, b = 1) { return a + b; };\nh(f(x));");
        let out = apply(&src).unwrap();
        assert!(out.contains("h(f(x))"), "{out}");
    }

    #[test]
    fn removes_a_definition_without_uses() {
        let src = wrap("C = class C0 { static only(r) { return r.kind; } static other() { return 1; } };\nC.only(x);");
        let out = apply(&src).unwrap();
        assert!(!out.contains("static only"), "{out}");
    }
}
