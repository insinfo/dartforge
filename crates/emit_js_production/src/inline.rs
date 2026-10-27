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
//! Two more bodies count as trivial: `let v = A; return B;`, where `B` reads
//! `v` once and before anything else (the getters of `dart:_rti`: `let s =
//! rti[_canonicalRecipe]; return asString(s);`), becomes `B` with `(A)` in
//! place of `v`; and a single expression statement (`rti[_kind] = kind;`,
//! `stack.push(v);`) is inlined only at a call whose value is dropped. A name
//! that is never assigned and always declared with a value is stable: as an
//! argument it may be evaluated after the body's first reads, and a body read
//! of it before a parameter does not force simple arguments.
//! `DARTFORGE_JSPROD_INLINE_FORMS=0` keeps only `return E;`, and
//! `DARTFORGE_JSPROD_STABLE_NAMES=0` treats no name as stable.
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
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Class, ClassElement,
    Expression, FormalParameters, Function, FunctionBody, MethodDefinitionKind, ObjectPropertyKind,
    PropertyKey, Statement, UnaryOperator,
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
    /// The inlined expression's text (the returned expression, or the
    /// fusion of `let v = A; return B;`).
    text: String,
    /// Only at calls whose value is discarded (a body that is one
    /// expression statement, which returns `undefined`).
    void_only: bool,
    /// Free names read before the last parameter: the order only holds when
    /// none of them is ever assigned ([`Scan::stable`]).
    early_names: Vec<String>,
    /// Number of parameters.
    arity: usize,
    /// Which parameters the body uses. A call may leave out trailing unused
    /// parameters, and the argument of an unused one must be [`droppable`].
    used: Vec<bool>,
    /// Offsets of the parameter uses inside `text`, with the parameter index.
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
        } else if c == b'(' || c == b')' {
            // The inlining wraps in parentheses that the printer drops: a run
            // of them counts once.
            n += 1;
            while i < b.len() && b[i] == c {
                i += 1;
            }
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
    /// Offset of the analyzed text: the recorded uses are relative to it.
    base: u32,
    uses: Vec<(u32, u32, usize)>,
    free: Vec<String>,
    /// Something other than a parameter or a free name was evaluated (a
    /// property read, a call, an operator).
    evaluated: bool,
    /// Free names read so far.
    read_names: Vec<String>,
    /// Free names read before some parameter.
    early_names: Vec<String>,
    /// A body that is one statement may assign a member (`r[k] = v`).
    assign: bool,
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
                    if self.evaluated
                        || self.conditional > 0
                        || self.last_param.is_some_and(|l| l > k)
                    {
                        self.simple_args = true;
                    } else {
                        self.early_names.extend(self.read_names.iter().cloned());
                    }
                    self.last_param = Some(k);
                    self.uses
                        .push((i.span.start - self.base, i.span.end - self.base, k));
                } else {
                    self.free.push(i.name.to_string());
                    self.read_names.push(i.name.to_string());
                }
                true
            }
            // `r[k] = v` / `r.k = v` as the whole body of a statement-bodied
            // function: the target's parts, then the value, then the write.
            Expression::AssignmentExpression(a)
                if self.assign
                    && a.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
            {
                self.assign = false;
                let target = match &a.left {
                    AssignmentTarget::StaticMemberExpression(m) if !m.optional => {
                        self.expr(&m.object)
                    }
                    AssignmentTarget::ComputedMemberExpression(m) if !m.optional => {
                        self.expr(&m.object) && self.expr(&m.expression)
                    }
                    _ => false,
                };
                if !target || !self.expr(&a.right) {
                    return false;
                }
                self.evaluated = true;
                true
            }
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
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
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => {
                    false
                }
                x => x.as_expression().is_some_and(|x| self.expr(x)),
            }),
            Expression::ObjectExpression(o) => o.properties.iter().all(|p| match p {
                ObjectPropertyKind::ObjectProperty(p) => {
                    !p.computed && !p.method && !p.shorthand && self.expr(&p.value)
                }
                ObjectPropertyKind::SpreadProperty(_) => false,
            }),
            _ => false,
        }
    }

    fn arguments(&mut self, args: &[Argument<'_>]) -> bool {
        args.iter()
            .all(|a| a.as_expression().is_some_and(|x| self.expr(x)))
    }
}

/// An argument whose value and effects cannot change when body evaluation
/// moves before it: [`droppable`], or a name that is never assigned and is
/// always declared with a value ([`Scan::stable`]).
fn stable_argument(
    e: &Expression<'_>,
    stable: &HashSet<String>,
    scoping: Option<&oxc_semantic::Scoping>,
) -> bool {
    match e {
        Expression::Identifier(i) if stable.contains(i.name.as_str()) => true,
        // A binding with no write anywhere (a parameter, a `let x = …` never
        // assigned again): the same value before and after the body's reads.
        Expression::Identifier(i) => scoping.is_some_and(|s| {
            i.reference_id
                .get()
                .and_then(|r| s.get_reference(r).symbol_id())
                .is_some_and(|sym| !s.get_resolved_references(sym).any(|r| r.is_write()))
        }),
        Expression::ParenthesizedExpression(p) => stable_argument(&p.expression, stable, scoping),
        e => literal_argument(e),
    }
}

/// A literal, `this` or a deduplicated *string*: evaluating it has no effect
/// and cannot throw.
fn literal_argument(e: &Expression<'_>) -> bool {
    match e {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::ThisExpression(_) => true,
        Expression::ParenthesizedExpression(p) => literal_argument(&p.expression),
        // A deduplicated *string* (`minify::dedup_strings`): a `var` with a
        // literal value, declared and initialized first thing in the file.
        Expression::Identifier(i) => i
            .name
            .strip_prefix("t$S")
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit())),
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
            BindingPattern::BindingIdentifier(b) => {
                Some((b.name.to_string(), x.initializer.is_some()))
            }
            _ => None,
        })
        .collect()
}

/// An unused argument can disappear only when evaluation cannot throw or
/// invoke an accessor. A bare name needs a separate initialization proof.
fn droppable(e: &Expression<'_>) -> bool {
    literal_argument(e)
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

/// The expression of a body that is a single expression statement (the
/// function returns `undefined`: inlined only where the value is dropped).
fn single_statement<'b, 'a>(body: &'b FunctionBody<'a>) -> Option<&'b Expression<'a>> {
    if !body.directives.is_empty() || body.statements.len() != 1 {
        return None;
    }
    match &body.statements[0] {
        Statement::ExpressionStatement(e) => Some(&e.expression),
        _ => None,
    }
}

/// `let v = A; return B;` where `B` reads `v` once and evaluates it before
/// anything else: the text of `B` with `(A)` in place of `v` (the same
/// evaluation order).
fn fused_return(source: &str, body: &FunctionBody<'_>) -> Option<String> {
    if !body.directives.is_empty() || body.statements.len() != 2 {
        return None;
    }
    let Statement::VariableDeclaration(d) = &body.statements[0] else {
        return None;
    };
    let Statement::ReturnStatement(r) = &body.statements[1] else {
        return None;
    };
    let ([v], Some(b)) = (&d.declarations[..], r.argument.as_ref()) else {
        return None;
    };
    let (BindingPattern::BindingIdentifier(name), Some(a)) = (&v.id, v.init.as_ref()) else {
        return None;
    };
    let name = name.name.as_str();
    let first = first_evaluated(b)?;
    if first.name.as_str() != name {
        return None;
    }
    let mut count = NameCount {
        name,
        count: 0,
        nested: false,
    };
    count.visit_expression(b);
    if count.count != 1 || count.nested {
        return None;
    }
    let bs = b.span();
    let at = |x: u32| x as usize;
    let a = &source[at(a.span().start)..at(a.span().end)];
    Some(format!(
        "{}({a}){}",
        &source[at(bs.start)..at(first.span.start)],
        &source[at(first.span.end)..at(bs.end)]
    ))
}

/// The leftmost leaf `e` evaluates first, when it is a name.
fn first_evaluated<'b>(e: &'b Expression<'_>) -> Option<&'b oxc_ast::ast::IdentifierReference<'b>> {
    match e {
        Expression::Identifier(i) => Some(i),
        Expression::ParenthesizedExpression(p) => first_evaluated(&p.expression),
        Expression::StaticMemberExpression(m) => first_evaluated(&m.object),
        Expression::ComputedMemberExpression(m) => first_evaluated(&m.object),
        Expression::CallExpression(c) => first_evaluated(&c.callee),
        Expression::BinaryExpression(b) => first_evaluated(&b.left),
        Expression::LogicalExpression(l) => first_evaluated(&l.left),
        Expression::ConditionalExpression(c) => first_evaluated(&c.test),
        Expression::UnaryExpression(u) if u.operator != UnaryOperator::Delete => {
            first_evaluated(&u.argument)
        }
        _ => None,
    }
}

/// How many times an expression reads `name` (and whether some function
/// inside it could).
struct NameCount<'n> {
    name: &'n str,
    count: usize,
    nested: bool,
}

impl<'a> Visit<'a> for NameCount<'_> {
    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if it.name.as_str() == self.name {
            self.count += 1;
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.nested = true;
    }
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
        self.nested = true;
    }
}

/// The names a binding pattern declares.
fn binding_names(p: &BindingPattern<'_>, out: &mut Vec<String>) {
    match p {
        BindingPattern::BindingIdentifier(b) => out.push(b.name.to_string()),
        BindingPattern::ObjectPattern(o) => {
            for x in o.properties.iter() {
                binding_names(&x.value, out);
            }
            if let Some(r) = &o.rest {
                binding_names(&r.argument, out);
            }
        }
        BindingPattern::ArrayPattern(a) => {
            for x in a.elements.iter().flatten() {
                binding_names(x, out);
            }
            if let Some(r) = &a.rest {
                binding_names(&r.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(a) => binding_names(&a.left, out),
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
    /// Names some declarator leaves without a value (`var x;`, the `for-of`
    /// heads), or that some `var` declares twice.
    unset: HashSet<String>,
    /// `var` declarators with a value, per name.
    var_inits: HashMap<String, usize>,
    /// Every declared name.
    declared: HashSet<String>,
}

impl<'s> Scan<'s> {
    fn text(&self, s: oxc_span::Span) -> &'s str {
        &self.source[s.start as usize..s.end as usize]
    }

    /// Names whose every binding is declared with a value and never assigned:
    /// reading one has no effect and always gives the same value, so a call
    /// argument or a body read of it can move across other evaluation.
    fn stable(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for n in self.declared.iter() {
            if !self.unset.contains(n)
                && !self.assigned.contains_key(n)
                && self.var_inits.get(n).is_none_or(|&k| k < 2)
            {
                out.insert(n.clone());
            }
        }
        out
    }

    fn define(
        &mut self,
        key: String,
        params: Option<Vec<(String, bool)>>,
        body: Option<&FunctionBody<'_>>,
        own: (u32, u32),
        removal: (u32, u32, String),
        from_assignment: bool,
    ) {
        if self.defs.contains_key(&key) || self.refused.contains(&key) {
            self.defs.remove(&key);
            self.refused.insert(key);
            return;
        }
        let (Some(params), Some(body)) = (params, body) else {
            self.refused.insert(key);
            return;
        };
        // The three trivial forms: `return E;`, `let v = A; return B;` (the
        // fused text is parsed on its own) and `E;` (`DARTFORGE_JSPROD_INLINE_FORMS=0`
        // keeps only the first).
        let forms = std::env::var("DARTFORGE_JSPROD_INLINE_FORMS").map_or(true, |v| v != "0");
        let fused = if forms && single_return(body).is_none() {
            fused_return(self.source, body)
        } else {
            None
        };
        let allocator = oxc_allocator::Allocator::default();
        let parsed_fused = fused.as_ref().map(|t| {
            let wrapped = format!("({t});");
            let text = allocator.alloc_str(&wrapped);
            oxc_parser::Parser::new(&allocator, text, oxc_span::SourceType::cjs()).parse()
        });
        let (e, void_only, text_of): (&Expression<'_>, bool, Option<&str>) =
            if let Some(e) = single_return(body) {
                (e, false, None)
            } else if let Some(p) = parsed_fused.as_ref() {
                let e = match p.program.body.first() {
                    Some(Statement::ExpressionStatement(x)) if p.diagnostics.is_empty() => {
                        &x.expression
                    }
                    _ => {
                        self.refused.insert(key);
                        return;
                    }
                };
                (e, false, Some(p.program.source_text))
            } else if let Some(e) = single_statement(body).filter(|_| forms) {
                (e, true, None)
            } else {
                self.refused.insert(key);
                return;
            };
        let span = e.span();
        if (span.end - span.start) as usize > MAX_BODY {
            self.refused.insert(key);
            return;
        }
        let text = match text_of {
            Some(t) => t[span.start as usize..span.end as usize].to_string(),
            None => self.source[span.start as usize..span.end as usize].to_string(),
        };
        let (params, defaults): (Vec<String>, Vec<bool>) = params.into_iter().unzip();
        let mut b = Body {
            params: &params,
            base: span.start,
            uses: Vec::new(),
            free: Vec::new(),
            evaluated: false,
            read_names: Vec::new(),
            early_names: Vec::new(),
            assign: void_only,
            conditional: 0,
            simple_args: false,
            last_param: None,
        };
        // `arguments` would be the caller's after the inlining.
        if !b.expr(e) || b.free.iter().any(|n| n == "arguments") {
            self.refused.insert(key);
            return;
        }
        let used: Vec<bool> = (0..params.len())
            .map(|k| b.uses.iter().any(|u| u.2 == k))
            .collect();
        // A default value only matters when the parameter is read.
        if used.iter().zip(&defaults).any(|(&u, &d)| u && d) {
            self.refused.insert(key);
            return;
        }
        let body_size = minified_estimate(&text).saturating_sub(2 * b.uses.len());
        let call_size = minified_estimate(&key) + 2 + params.len().saturating_sub(1);
        let def = Def {
            text,
            void_only,
            early_names: b.early_names,
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
            let ClassElement::MethodDefinition(m) = el else {
                continue;
            };
            if !m.r#static || m.computed || m.kind != MethodDefinitionKind::Method {
                continue;
            }
            let PropertyKey::StaticIdentifier(k) = &m.key else {
                continue;
            };
            let f = &m.value;
            if f.generator || f.r#async {
                continue;
            }
            let key = format!("{owner}.{}", k.name);
            self.define(
                key,
                plain_params(&f.params),
                f.body.as_deref(),
                (m.span.start, m.span.end),
                (m.span.start, m.span.end, k.name.to_string()),
                false,
            );
        }
    }

    fn function_value(
        &mut self,
        name: &str,
        f: &Function<'_>,
        own: (u32, u32),
        removal: (u32, u32, String),
        from_assignment: bool,
    ) {
        if f.generator || f.r#async {
            self.refused.insert(name.to_string());
            return;
        }
        self.define(
            name.to_string(),
            plain_params(&f.params),
            f.body.as_deref(),
            own,
            removal,
            from_assignment,
        );
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
        if self.depth <= 2
            && let Some(b) = &it.body
        {
            for s in b.statements.iter() {
                self.top_statements.insert(s.span().start);
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
                        let BindingPattern::BindingIdentifier(b) = &v.id else {
                            continue;
                        };
                        let name = b.name.to_string();
                        self.top_names
                            .entry(name.clone())
                            .or_default()
                            .insert(scope);
                        self.top_bindings.insert(b.span.start);
                        *self.top_decls.entry(name.clone()).or_default() += 1;
                        match &v.init {
                            Some(Expression::ClassExpression(c)) => self.class_statics(&name, c),
                            // A `var` declarator is never removed (the statement may hold others).
                            Some(Expression::FunctionExpression(f)) => self.function_value(
                                &name,
                                f,
                                (f.span.start, f.span.end),
                                (0, 0, String::new()),
                                false,
                            ),
                            _ => {}
                        }
                    }
                }
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        let name = id.name.to_string();
                        self.top_names
                            .entry(name.clone())
                            .or_default()
                            .insert(scope);
                        self.top_bindings.insert(id.span.start);
                        self.function_value(
                            &name,
                            f,
                            (f.span.start, f.span.end),
                            (f.span.start, f.span.end, name.clone()),
                            false,
                        );
                    }
                }
                Statement::ExpressionStatement(s) => {
                    if let Expression::AssignmentExpression(a) = &s.expression {
                        let target = match &a.left {
                            AssignmentTarget::AssignmentTargetIdentifier(i) => {
                                Some(i.name.to_string())
                            }
                            AssignmentTarget::StaticMemberExpression(m) => {
                                Some(self.text(m.span).to_string())
                            }
                            _ => None,
                        };
                        if let Some(t) = target {
                            match &a.right {
                                Expression::FunctionExpression(f) => self.function_value(
                                    &t,
                                    f,
                                    (f.span.start, f.span.end),
                                    (s.span.start, s.span.end, t.clone()),
                                    true,
                                ),
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
        self.declared.insert(it.name.to_string());
    }
    fn visit_variable_declaration(&mut self, it: &oxc_ast::ast::VariableDeclaration<'a>) {
        for d in it.declarations.iter() {
            let mut names = Vec::new();
            binding_names(&d.id, &mut names);
            for n in names {
                if d.init.is_none() {
                    self.unset.insert(n);
                } else if it.kind == oxc_ast::ast::VariableDeclarationKind::Var {
                    *self.var_inits.entry(n).or_default() += 1;
                }
            }
        }
        walk::walk_variable_declaration(self, it);
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
    /// [`Scan::stable`].
    stable: &'s HashSet<String>,
    /// The file's bindings (`None` with `DARTFORGE_JSPROD_STABLE_NAMES=0`).
    scoping: Option<&'s oxc_semantic::Scoping>,
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
    fn visit_expression_statement(&mut self, it: &oxc_ast::ast::ExpressionStatement<'a>) {
        let mut e = &it.expression;
        while let Expression::ParenthesizedExpression(p) = e {
            e = &p.expression;
        }
        if let Expression::CallExpression(c) = e
            && c.span.start >= self.covered
            && let Some(new) = self.inline(c, true)
        {
            self.edits.push((c.span.start, c.span.end, new));
            self.covered = c.span.end;
            return;
        }
        walk::walk_expression_statement(self, it);
    }
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if it.span.start >= self.covered
            && let Some(new) = self.inline(it, false)
        {
            self.edits.push((it.span.start, it.span.end, new));
            self.covered = it.span.end;
            return;
        }
        walk::walk_call_expression(self, it);
    }
}

impl Sites<'_> {
    fn inline(&self, it: &oxc_ast::ast::CallExpression<'_>, statement: bool) -> Option<String> {
        if it.optional {
            return None;
        }
        let key = match &it.callee {
            Expression::Identifier(i) => i.name.as_str(),
            Expression::StaticMemberExpression(m)
                if !m.optional && matches!(m.object, Expression::Identifier(_)) =>
            {
                &self.source[m.span.start as usize..m.span.end as usize]
            }
            _ => return None,
        };
        let d = self.defs.get(key)?;
        if d.void_only && !statement {
            return None;
        }
        // Fewer arguments: the parameters left out must be unused.
        if it.arguments.len() > d.arity
            || d.used[it.arguments.len()..].iter().any(|&u| u)
            || (it.span.start >= d.own.0 && it.span.end <= d.own.1)
        {
            return None;
        }
        // Free names declared at the top of an IIFE only mean the same in it.
        if d.iife.is_some() && d.iife != self.iife {
            return None;
        }
        let args: Vec<&Expression<'_>> = it
            .arguments
            .iter()
            .map(|a| a.as_expression())
            .collect::<Option<_>>()?;
        if d.simple_args
            && !args
                .iter()
                .all(|a| stable_argument(a, self.stable, self.scoping))
        {
            return self.drop_unused_tail(it, d, &args);
        }
        // The arguments of unused parameters are dropped.
        if args.iter().zip(&d.used).any(|(a, &u)| !u && !droppable(a)) {
            return None;
        }
        let mut out = String::from("(");
        let mut pos = 0u32;
        for &(s, e, k) in &d.uses {
            out.push_str(&d.text[pos as usize..s as usize]);
            let a = args[k].span();
            out.push('(');
            out.push_str(&self.source[a.start as usize..a.end as usize]);
            out.push(')');
            pos = e;
        }
        out.push_str(&d.text[pos as usize..]);
        out.push(')');
        Some(out)
    }
}

impl Sites<'_> {
    /// A call that cannot be inlined still loses the trailing arguments of
    /// parameters the body never reads, when they have no effect
    /// ([`droppable`]): the `checkBinding(old, new, "expr", location)` of the
    /// ngdart views, whose body reads two (3,370 calls in limitless_ui).
    fn drop_unused_tail(
        &self,
        it: &oxc_ast::ast::CallExpression<'_>,
        d: &Def,
        args: &[&Expression<'_>],
    ) -> Option<String> {
        if std::env::var("DARTFORGE_JSPROD_DROP_UNUSED_ARGS").is_ok_and(|v| v == "0") {
            return None;
        }
        let mut keep = args.len();
        while keep > 0 && !d.used[keep - 1] && droppable(args[keep - 1]) {
            keep -= 1;
        }
        if keep == args.len() {
            return None;
        }
        let callee = it.callee.span();
        let mut out = self.source[callee.start as usize..callee.end as usize].to_string();
        out.push('(');
        let kept: Vec<&str> = args[..keep]
            .iter()
            .map(|a| {
                let s = a.span();
                &self.source[s.start as usize..s.end as usize]
            })
            .collect();
        out.push_str(&kept.join(", "));
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
        unset: HashSet::new(),
        var_inits: HashMap::new(),
        declared: HashSet::new(),
    };
    scan.visit_program(&parsed.program);
    // Free names that are not declared at all are globals (`Array`, `Object`):
    // the file never assigns them either.
    // `DARTFORGE_JSPROD_STABLE_NAMES=0`: no name is stable.
    let names_on = std::env::var("DARTFORGE_JSPROD_STABLE_NAMES").map_or(true, |v| v != "0");
    let mut stable = if names_on {
        scan.stable()
    } else {
        HashSet::new()
    };
    for d in scan.defs.values().filter(|_| names_on) {
        for n in d.free.iter() {
            if !scan.declared.contains(n) && !scan.assigned.contains_key(n) {
                stable.insert(n.clone());
            }
        }
    }
    let mut defs = std::mem::take(&mut scan.defs);
    defs.retain(|key, d| {
        // Assigned again somewhere (`C.m = …`, a second `f = …`).
        if scan.assigned.get(key).copied().unwrap_or(0) > usize::from(d.from_assignment) {
            return false;
        }
        d.free.iter().all(|n| {
            !scan.locals.contains(n)
                && scan
                    .top_names
                    .get(n)
                    .is_none_or(|s| s.len() == 1 && (s.contains(&None) || s.contains(&d.iife)))
        })
    });
    for (key, d) in defs.iter_mut() {
        // A free name read before a parameter keeps the order only when it
        // cannot change in between.
        if d.early_names.iter().any(|n| !stable.contains(n)) {
            d.simple_args = true;
        }
        // A definition whose free names are all global or of the file's top
        // can be inlined anywhere.
        if d.free
            .iter()
            .all(|n| scan.top_names.get(n).is_none_or(|s| s.contains(&None)))
        {
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
        let calls = counts
            .get(name)
            .copied()
            .unwrap_or(0)
            .saturating_sub(d.allowed);
        let removed = if d.removal.2.is_empty() {
            0
        } else {
            d.def_size
        };
        calls * d.call_size + removed >= calls * d.body_size
    });
    let semantic = names_on.then(|| {
        oxc_semantic::SemanticBuilder::new()
            .build(&parsed.program)
            .semantic
    });
    let mut sites = Sites {
        source,
        defs: &defs,
        stable: &stable,
        scoping: semantic.as_ref().map(|s| s.scoping()),
        depth: 0,
        iife: None,
        iifes: 0,
        edits: Vec::new(),
        covered: 0,
    };
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
        let src = wrap(
            "f = function(a, b, c = null, d = null) { return !g(a, b); };\nh(f(x.y, z, \"expr\", loc));\nh(f(x, z));\nh(f(x, z, w.v));\nh(f(1, 2, \"expr\", 5));\nh(f(x.y, z, \"expr\", t$S3));",
        );
        let out = apply(&src).unwrap();
        // The callee is looked up in the body; mutable arguments stay eager.
        assert!(out.contains("h(f(x.y, z, \"expr\", loc))"), "{out}");
        assert!(out.contains("h(f(x, z))"), "{out}");
        assert!(out.contains("h((!g((1), (2))))"), "{out}");
        // A property read can throw: its argument is not dropped.
        assert!(out.contains("h(f(x, z, w.v))"), "{out}");
        // Not inlined (the arguments are not stable), the call still loses
        // the arguments of the unused parameters that cannot throw.
        assert!(out.contains("h(f(x.y, z));"), "{out}");
    }

    #[test]
    #[ignore = "requires Node on PATH"]
    fn unused_argument_preserves_temporal_dead_zone_error() {
        let source = wrap(
            "f = function(unused) { return 7; };\ntry { console.log(f(value)); } catch (error) { console.log(error.name); }\nlet value = 1;",
        );
        let transformed = apply(&source).unwrap();
        for code in [&source, &transformed] {
            let output = std::process::Command::new("node")
                .args(["-e", code])
                .output()
                .unwrap();
            assert!(output.status.success() && output.stderr.is_empty());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                "ReferenceError\n"
            );
        }
    }

    #[test]
    fn refuses_a_used_parameter_with_a_default() {
        let src = wrap("f = function(a, b = 1) { return a + b; };\nh(f(x));");
        let out = apply(&src).unwrap();
        assert!(out.contains("h(f(x))"), "{out}");
    }

    #[test]
    fn fuses_a_local_returned_right_away() {
        let src = wrap("C = class C0 { static k(r) { let s = r.kind; return (s); } };\nh(C.k(x));");
        let out = apply(&src).unwrap();
        assert!(out.contains("(x).kind") && !out.contains("C.k("), "{out}");
        // A body that evaluates something before the local keeps the call.
        let src =
            wrap("C = class C0 { static k(r) { let s = r.kind; return g(s); } };\nh(C.k(x));");
        let out = apply(&src).unwrap();
        assert!(out.contains("h(C.k(x))"), "{out}");
    }

    #[test]
    fn inlines_a_statement_body_only_where_the_value_is_dropped() {
        let src =
            wrap("C = class C0 { static set(r, v) { r.k = v; } };\nC.set(a, 1);\nh(C.set(a, 2));");
        let out = apply(&src).unwrap();
        assert!(out.contains("((a).k = (1));"), "{out}");
        assert!(out.contains("h(C.set(a, 2))"), "{out}");
    }

    #[test]
    fn moves_a_body_read_only_across_names_never_assigned() {
        // `m.get` is read before `k`: the argument must not change in between.
        let src = wrap(
            "f = function(m, k) { return m.get(k); };\nfunction q(mm) { let kk = 1; return f(mm, kk); }\nfunction w(mm) { let jj; jj = 2; return f(mm, jj); }",
        );
        let out = apply(&src).unwrap();
        assert!(out.contains("return ((mm).get((kk)))"), "{out}");
        assert!(out.contains("return f(mm, jj)"), "{out}");
        // By binding, not by name: `kk` is written in another function.
        let src = wrap(
            "f = function(m, k) { return m.get(k); };\nfunction q(mm, kk) { h(1); return f(mm, kk); }\nfunction w(mm) { let kk = 1; kk = 2; return f(mm, kk); }\nh(q, w);",
        );
        let out = apply(&src).unwrap();
        assert!(out.contains("return ((mm).get((kk)))"), "{out}");
        assert!(out.contains("return f(mm, kk)"), "{out}");
    }

    #[test]
    fn removes_a_definition_without_uses() {
        let src = wrap(
            "C = class C0 { static only(r) { return r.kind; } static other() { return 1; } };\nC.only(x);",
        );
        let out = apply(&src).unwrap();
        assert!(!out.contains("static only"), "{out}");
    }
}
