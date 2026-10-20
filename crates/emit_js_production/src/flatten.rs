//! Library members as variables (`docs/JS-PRODUCAO-TAMANHO.md` §9.2).
//!
//! In the DDC contract each library is an object (`var core =
//! Object.create(dart.library)`) and each top-level member is a property of
//! it: `core.print(…)`, `dart.dsend(…)`, `L$pacote__lib.Classe.new(…)`. In a
//! single file, that object is observed by no one but the text itself, so
//! `N.x` can become the variable `N$$x`, declared once at the top of the
//! file. `oxc`'s identifier renaming then shortens it like a local (two
//! letters instead of `ns.prop`), and the name stops competing for the
//! property-name space: `dart.throw`, `dart.bind`, `html.document` and
//! ngdart's `setAttribute`, which stayed as they were because the same name
//! is a JavaScript or DOM property, get shortened too.
//!
//! The rule is conservative; each condition has its reason:
//!
//! * `N` is declared at the top of the file with `Object.create(…)` and no
//!   other *binding* in the file has the same name (otherwise the reference
//!   could be to the other one);
//! * `N` only appears as the object of `N.x`/`N["x"]`, or as the first
//!   argument of `dart.defineLazy`/`dart.copyProperties` (with an object
//!   literal as the second) and of `dart.privateName`. Any other use (passed
//!   to another function, `N[expr]`) may read the properties by name at run
//!   time, and the whole object stays as it is;
//! * `x` has at least one write `N.x = …` (otherwise the variable would never
//!   be declared) and is not a key of `N`'s `defineLazy`/`copyProperties`
//!   (it is a *getter* on the object);
//! * no function assigned to `N.x` reads `this`: called as `N.x()` it
//!   receives `N` as `this`, and as `N$$x()`, it does not. The same for
//!   `delete N.x`.
//!
//! The replacement is textual, through the spans `oxc` gives each access, as
//! in *string* deduplication. `DARTFORGE_JSPROD_FLATTEN=0` turns it off.

use oxc_ast::ast::{
    Argument, AssignmentOperator, AssignmentTarget, BindingIdentifier, CallExpression,
    ComputedMemberExpression, Expression, IdentifierReference, ObjectPropertyKind, PropertyKey,
    Statement, StaticMemberExpression, UnaryExpression, UnaryOperator, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use std::collections::{HashMap, HashSet};

/// Separator between the namespace and the member in the variable name. `$$`
/// does not appear in any name the emitter generates for a library.
pub const SEPARATOR: &str = "$$";

#[derive(Default)]
struct Namespace {
    /// `N.x`/`N["x"]` accesses: (name, start, end).
    accesses: Vec<(String, u32, u32)>,
    /// Members with a write `N.x = …`.
    written: HashSet<String>,
    /// Members that cannot become variables.
    pinned: HashSet<String>,
    /// The object is observed as a whole: nothing of it becomes a variable.
    escapes: bool,
    /// Where it first escaped (diagnostics, `DARTFORGE_JSPROD_FLATTEN_WHY`).
    escapes_at: Option<u32>,
}

struct Analysis<'s> {
    source: &'s str,
    ns: HashMap<String, Namespace>,
    /// How many times each name is declared (*binding*) in the file.
    bindings: HashMap<String, u32>,
    /// Every identifier of the file (to avoid collisions).
    identifiers: HashSet<String>,
    /// The deduplication `t$S…` whose value is not an identifier: as a key
    /// (`core[t$S5]`) they cannot cite a flattened member.
    opaque_keys: HashSet<String>,
}

/// Whether the function body reads `this` (outside nested functions and
/// classes, which have their own).
struct UsesThis(bool);

impl<'a> Visit<'a> for UsesThis {
    fn visit_this_expression(&mut self, _it: &oxc_ast::ast::ThisExpression) {
        self.0 = true;
    }
    fn visit_function(
        &mut self,
        _it: &oxc_ast::ast::Function<'a>,
        _flags: oxc_syntax::scope::ScopeFlags,
    ) {
    }
    fn visit_class(&mut self, _it: &oxc_ast::ast::Class<'a>) {}
}

fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

impl<'s> Analysis<'s> {
    fn target<'b>(&self, e: &'b Expression<'_>) -> Option<&'b str> {
        match e {
            Expression::Identifier(i) if self.ns.contains_key(i.name.as_str()) => {
                Some(i.name.as_str())
            }
            _ => None,
        }
    }

    /// `(N, x)` of an `N.x`/`N["x"]`.
    fn member_of(&self, e: &Expression<'_>) -> Option<(String, String)> {
        match e {
            Expression::StaticMemberExpression(m) => self
                .target(&m.object)
                .map(|n| (n.to_string(), m.property.name.to_string())),
            Expression::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) if is_identifier(s.value.as_str()) => self
                    .target(&m.object)
                    .map(|n| (n.to_string(), s.value.to_string())),
                _ => None,
            },
            _ => None,
        }
    }
}

impl<'a> Visit<'a> for Analysis<'_> {
    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        *self.bindings.entry(it.name.to_string()).or_default() += 1;
        self.identifiers.insert(it.name.to_string());
    }
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        let n = it.name.as_str();
        self.identifiers.insert(n.to_string());
        if let Some(ns) = self.ns.get_mut(n) {
            ns.escapes = true;
            ns.escapes_at.get_or_insert(it.span.start);
        }
    }
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if let Some(n) = self.target(&it.object).map(str::to_string) {
            let ns = self.ns.get_mut(&n).expect("namespace");
            ns.accesses
                .push((it.property.name.to_string(), it.span.start, it.span.end));
            return;
        }
        walk::walk_static_member_expression(self, it);
    }
    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if let Some(n) = self.target(&it.object).map(str::to_string) {
            match &it.expression {
                Expression::StringLiteral(s) if is_identifier(s.value.as_str()) => {
                    let ns = self.ns.get_mut(&n).expect("namespace");
                    ns.accesses
                        .push((s.value.to_string(), it.span.start, it.span.end));
                }
                // `core["EnumName|get#name"]`: a name that is never an
                // identifier stays a property of the object, which keeps
                // existing. The same through the deduplication `t$S…`.
                Expression::StringLiteral(_) => {}
                Expression::Identifier(i) if self.opaque_keys.contains(i.name.as_str()) => {}
                // `N[expr]`: the object is read by name at run time.
                _ => {
                    let ns = self.ns.get_mut(&n).expect("namespace");
                    ns.escapes = true;
                    ns.escapes_at.get_or_insert(it.span.start);
                    self.visit_expression(&it.expression);
                }
            }
            return;
        }
        walk::walk_computed_member_expression(self, it);
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let function = match &it.callee {
            Expression::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == "dart") => {
                Some(m.property.name.as_str())
            }
            _ => None,
        };
        let first = it
            .arguments
            .first()
            .and_then(|a| a.as_expression())
            .and_then(|e| self.target(e))
            .map(str::to_string);
        match (function, first) {
            (Some("defineLazy" | "copyProperties"), Some(n)) => {
                // The keys of the object literal become properties
                // (accessors) of `N`: they stay.
                match it.arguments.get(1).and_then(Argument::as_expression) {
                    Some(Expression::ObjectExpression(o)) => {
                        for p in &o.properties {
                            match p {
                                ObjectPropertyKind::ObjectProperty(p) => match &p.key {
                                    PropertyKey::StaticIdentifier(k) => {
                                        self.ns
                                            .get_mut(&n)
                                            .expect("namespace")
                                            .pinned
                                            .insert(k.name.to_string());
                                    }
                                    PropertyKey::StringLiteral(k) => {
                                        self.ns
                                            .get_mut(&n)
                                            .expect("namespace")
                                            .pinned
                                            .insert(k.value.to_string());
                                    }
                                    _ => self.ns.get_mut(&n).expect("namespace").escapes = true,
                                },
                                ObjectPropertyKind::SpreadProperty(_) => {
                                    self.ns.get_mut(&n).expect("namespace").escapes = true
                                }
                            }
                        }
                    }
                    _ => self.ns.get_mut(&n).expect("namespace").escapes = true,
                }
                self.visit_expression(&it.callee);
                for a in it.arguments.iter().skip(1) {
                    self.visit_argument(a);
                }
            }
            (Some("privateName"), Some(_)) => {
                self.visit_expression(&it.callee);
                for a in it.arguments.iter().skip(1) {
                    self.visit_argument(a);
                }
            }
            _ => walk::walk_call_expression(self, it),
        }
    }
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        let member = match &it.left {
            AssignmentTarget::StaticMemberExpression(m) => self
                .target(&m.object)
                .map(|n| (n.to_string(), m.property.name.to_string())),
            AssignmentTarget::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) if is_identifier(s.value.as_str()) => self
                    .target(&m.object)
                    .map(|n| (n.to_string(), s.value.to_string())),
                _ => None,
            },
            _ => None,
        };
        if let Some((n, x)) = member {
            let uses_this = it.operator == AssignmentOperator::Assign
                && match &it.right {
                    Expression::FunctionExpression(f) => {
                        let mut u = UsesThis(false);
                        if let Some(b) = &f.body {
                            u.visit_function_body(b);
                        }
                        u.0
                    }
                    _ => false,
                };
            let ns = self.ns.get_mut(&n).expect("namespace");
            ns.written.insert(x.clone());
            if uses_this {
                ns.pinned.insert(x);
            }
        }
        walk::walk_assignment_expression(self, it);
    }
    fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
        if it.operator == UnaryOperator::Delete
            && let Some((n, x)) = self.member_of(&it.argument)
        {
            self.ns.get_mut(&n).expect("namespace").pinned.insert(x);
        }
        walk::walk_unary_expression(self, it);
    }
}

/// The candidate namespaces: `var|let|const N = Object.create(…)` at the top
/// of the body of the function that wraps the file.
fn candidates(program: &oxc_ast::ast::Program<'_>) -> Vec<String> {
    fn body<'b, 'a>(stmts: &'b [Statement<'a>]) -> Option<&'b [Statement<'a>]> {
        for s in stmts {
            if let Statement::ExpressionStatement(e) = s
                && let Expression::CallExpression(c) = &e.expression
                && let Expression::ParenthesizedExpression(p) = &c.callee
                && let Expression::FunctionExpression(f) = &p.expression
            {
                return f.body.as_ref().map(|b| b.statements.as_slice());
            }
        }
        None
    }
    let Some(stmts) = body(&program.body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let is_create = |d: &VariableDeclarator<'_>| {
        matches!(&d.init, Some(Expression::CallExpression(c)) if matches!(&c.callee, Expression::StaticMemberExpression(m)
            if matches!(&m.object, Expression::Identifier(o) if o.name == "Object") && m.property.name == "create"))
    };
    for s in stmts {
        if let Statement::VariableDeclaration(v) = s {
            for d in &v.declarations {
                if let Some(id) = d.id.get_binding_identifier()
                    && is_create(d)
                {
                    out.push(id.name.to_string());
                }
            }
        }
    }
    out
}

/// The `var t$S… = "…"` at the top of the file (*string* deduplication)
/// whose value is not shaped like an identifier.
fn opaque_keys(program: &oxc_ast::ast::Program<'_>) -> HashSet<String> {
    struct V(HashSet<String>);
    impl<'a> Visit<'a> for V {
        fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
            if let Some(id) = it.id.get_binding_identifier()
                && id.name.starts_with("t$S")
                && let Some(Expression::StringLiteral(l)) = &it.init
                && !is_identifier(l.value.as_str())
            {
                self.0.insert(id.name.to_string());
            }
        }
        fn visit_function_body(&mut self, _it: &oxc_ast::ast::FunctionBody<'a>) {}
    }
    let mut v = V(HashSet::new());
    // Only the top: the file's wrapper and its body.
    for s in &program.body {
        if let Statement::ExpressionStatement(e) = s
            && let Expression::CallExpression(c) = &e.expression
            && let Expression::ParenthesizedExpression(p) = &c.callee
            && let Expression::FunctionExpression(f) = &p.expression
            && let Some(b) = &f.body
        {
            for st in &b.statements {
                if let Statement::VariableDeclaration(d) = st {
                    for decl in &d.declarations {
                        v.visit_variable_declarator(decl);
                    }
                }
            }
        }
    }
    v.0
}

/// Applies the replacement to `source` (the file already wrapped in
/// `(function () {…})();`). Returns the new source and how many members
/// became variables.
///
/// # Errors
///
/// The text is not valid JS for the parser.
pub fn apply(source: &str) -> Result<(String, usize), String> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let mut a = Analysis {
        source,
        ns: HashMap::new(),
        bindings: HashMap::new(),
        identifiers: HashSet::new(),
        opaque_keys: opaque_keys(&parsed.program),
    };
    for n in candidates(&parsed.program) {
        a.ns.insert(n, Namespace::default());
    }
    if a.ns.is_empty() {
        return Ok((source.to_string(), 0));
    }
    a.visit_program(&parsed.program);
    let _ = a.source;
    // Replacement by span, and the declarations at the start of the body.
    let mut edits: Vec<(u32, u32, String)> = Vec::new();
    let mut declared: Vec<String> = Vec::new();
    let mut names: Vec<&String> = a.ns.keys().collect();
    names.sort();
    let diag = std::env::var("DARTFORGE_JSPROD_FLATTEN_WHY").ok();
    for n in names {
        let ns = &a.ns[n];
        if diag
            .as_deref()
            .is_some_and(|d| d.split(',').any(|x| x == n) || (d == "*" && ns.escapes))
        {
            let snippet = ns
                .escapes_at
                .map(|i| &source[i as usize..(i as usize + 120).min(source.len())]);
            eprintln!(
                "[jsprod] flatten {n}: escapes={} bindings={:?} at {:?}",
                ns.escapes,
                a.bindings.get(n),
                snippet
            );
        }
        if ns.escapes || a.bindings.get(n).copied().unwrap_or(0) != 1 {
            continue;
        }
        let mut members: HashSet<&str> = HashSet::new();
        for x in &ns.written {
            let var = format!("{n}{SEPARATOR}{x}");
            if !ns.pinned.contains(x) && !a.identifiers.contains(&var) {
                members.insert(x.as_str());
            }
        }
        if diag.is_some() {
            let mut uses: HashMap<&str, usize> = HashMap::new();
            for (x, _, _) in &ns.accesses {
                *uses.entry(x.as_str()).or_default() += 1;
            }
            let dead: Vec<&&str> = members
                .iter()
                .filter(|x| uses.get(**x).copied().unwrap_or(0) <= 1)
                .collect();
            if !dead.is_empty() {
                eprintln!(
                    "[jsprod] flatten {n}: {} member(s) only written: {:?}",
                    dead.len(),
                    dead.iter().take(8).collect::<Vec<_>>()
                );
            }
        }
        let mut sorted: Vec<&&str> = members.iter().collect();
        sorted.sort();
        declared.extend(sorted.iter().map(|x| format!("{n}{SEPARATOR}{x}")));
        for (x, i, f) in &ns.accesses {
            if members.contains(x.as_str()) {
                edits.push((*i, *f, format!("{n}{SEPARATOR}{x}")));
            }
        }
    }
    if declared.is_empty() {
        return Ok((source.to_string(), 0));
    }
    edits.sort_by_key(|t| t.0);
    let head = "(function () {\n";
    if !source.starts_with(head) {
        return Ok((source.to_string(), 0));
    }
    let mut out = String::with_capacity(source.len());
    out.push_str(head);
    out.push_str("var ");
    out.push_str(&declared.join(", "));
    out.push_str(";\n");
    let mut pos = head.len();
    for (i, f, name) in edits {
        let (i, f) = (i as usize, f as usize);
        if i < pos {
            continue;
        }
        out.push_str(&source[pos..i]);
        out.push_str(&name);
        pos = f;
    }
    out.push_str(&source[pos..]);
    Ok((out, declared.len()))
}

/// The member `x` of a flattened name `N$$x` whose namespace is `ns`.
pub fn member<'n>(name: &'n str, ns: &str) -> Option<&'n str> {
    name.strip_prefix(ns)?.strip_prefix(SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(js: &str) -> String {
        apply(&format!("(function () {{\n{js}\n}})();\n"))
            .unwrap()
            .0
    }

    #[test]
    fn written_member_becomes_variable() {
        let s = run(
            "const dart = Object.create(null);\nvar core = Object.create(dart);\ncore.f = function(a) { return a; };\ncore.f(1);",
        );
        assert!(s.contains("var core$$f;"), "{s}");
        assert!(s.contains("core$$f = function(a)"), "{s}");
        assert!(s.contains("core$$f(1)"), "{s}");
    }

    #[test]
    fn observed_object_stays() {
        let s = run("var core = Object.create(null);\ncore.f = 1;\nlet k = 'f'; core[k];");
        assert!(!s.contains("core$$f"), "{s}");
        let s = run("var core = Object.create(null);\ncore.f = 1;\nObject.keys(core);");
        assert!(!s.contains("core$$f"), "{s}");
    }

    #[test]
    fn lazy_this_delete_and_unwritten_stay() {
        let s = run(
            "const dart = Object.create(null);\nvar L = Object.create(null);\ndart.defineLazy = function(a, b) {};\ndart.defineLazy(L, {get x() { return 1; }});\nL.x = 2;\nL.m = function() { return this; };\nL.d = 1; delete L.d;\nL.y;\nL.ok = () => 1;",
        );
        assert!(!s.contains("L$$x"), "{s}");
        assert!(!s.contains("L$$m"), "{s}");
        assert!(!s.contains("L$$d"), "{s}");
        assert!(!s.contains("L$$y"), "{s}");
        assert!(s.contains("L$$ok = () => 1"), "{s}");
        assert!(s.contains("dart$$defineLazy(L, {"), "{s}");
    }

    #[test]
    fn shadowed_name_stays() {
        let s = run(
            "var core = Object.create(null);\ncore.f = 1;\nfunction g(core) { return core.f; }",
        );
        assert!(!s.contains("core$$f"), "{s}");
    }
}
