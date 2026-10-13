//! The fourth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `prefer_if_null_operators`,
//! `avoid_single_cascade_in_expression_statements`,
//! `use_function_type_syntax_for_parameters`,
//! `no_leading_underscores_for_local_identifiers` and
//! `prefer_function_declarations_over_variables`.
//!
//! Written with the emitters of the SDK's `main` open and then checked
//! against those of 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`,
//! extracted from the tag on 2026-10-05). In 3.6.2,
//! `use_function_type_syntax_for_parameters` only looks at the plain
//! parameter (`FunctionTypedFormalParameter`), not `this.f(…)` nor
//! `super.f(…)`. `no_leading_underscores_for_local_identifiers` also looks at
//! a collection `for`/`for-in`, declaration pattern variables and the
//! parameter lists of `Function(…)` types and old-style typedefs;
//! `prefer_function_declarations_over_variables` reports outside a function
//! body only a `final` list (`isFinal`, without `const`) and, inside, a local
//! that no write in the unit changes (`isPotentiallyMutatedInScope`, by
//! element: it needs the unit's semantics); `prefer_if_null_operators`
//! compares the nodes' `toString` (`dartforge_frontend::fonte`).
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, BinaryOp, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, MemberKind, ParameterKind, StmtKind, VariableList,
};
use dartforge_intern::Interner;

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    // The `toString` of an expression.
    let source_of = |e: ExprId| -> String { dartforge_frontend::fonte::de_expr(a, source, interner, e) };
    // All the unit's parameter lists, with the nested ones.
    fn collect_lists<'x>(list: &'x [ast::Parameter], out: &mut Vec<&'x [ast::Parameter]>) {
        out.push(list);
        for p in list {
            if let Some(inner) = &p.function_parameters {
                collect_lists(inner, out);
            }
        }
    }
    let mut param_lists: Vec<&[ast::Parameter]> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = &f.parameters {
            collect_lists(ps, &mut param_lists);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            collect_lists(&k.parameters, &mut param_lists);
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Typedef(x) = &d.kind
            && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
        {
            collect_lists(parameters, &mut param_lists);
        }
    }
    for t in a.types.iter() {
        if let ast::TypeKind::Function { parameters, .. } = &t.kind {
            collect_lists(parameters, &mut param_lists);
        }
    }

    // `prefer_if_null_operators`: `a == null ? b : a` and `a != null ? a : b`.
    if enabled("prefer_if_null_operators") {
        for e in a.exprs.iter() {
            let ExprKind::Conditional { condition, then, else_ } = &e.kind else { continue };
            let ExprKind::Binary { op, left, right } = &a.expr(*condition).kind else { continue };
            if !matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                continue;
            }
            let tested = if matches!(a.expr(*left).kind, ExprKind::Null) {
                *right
            } else if matches!(a.expr(*right).kind, ExprKind::Null) {
                *left
            } else {
                continue;
            };
            let branch = if *op == BinaryOp::Eq { *else_ } else { *then };
            if source_of(branch) == source_of(tested) {
                report(&c::PREFER_IF_NULL_OPERATORS, e.span, &[]);
            }
        }
    }
    // `avoid_single_cascade_in_expression_statements`: a lone `a..b();`.
    if enabled("avoid_single_cascade_in_expression_statements") {
        for s in a.stmts.iter() {
            if let StmtKind::Expression(e) = &s.kind
                && let ExprKind::Cascade { sections, null_aware, .. } = &a.expr(*e).kind
                && sections.len() == 1
            {
                report(&c::AVOID_SINGLE_CASCADE_IN_EXPRESSION_STATEMENTS, a.expr(*e).span, &[if *null_aware { "?." } else { "." }]);
            }
        }
    }
    // `use_function_type_syntax_for_parameters`: `void f(int g(int x))`.
    if enabled("use_function_type_syntax_for_parameters") {
        for list in &param_lists {
            for p in list.iter().filter(|p| p.function_parameters.is_some() && !p.this_ && !p.super_) {
                report(&c::USE_FUNCTION_TYPE_SYNTAX_FOR_PARAMETERS, p.span, &[p.name.map_or("", |n| interner.resolve(n.sym))]);
            }
        }
    }
    // `no_leading_underscores_for_local_identifiers`.
    if enabled("no_leading_underscores_for_local_identifiers") {
        let mut check = |n: ast::Name| {
            let text = interner.resolve(n.sym);
            if text.starts_with('_') && !text.bytes().all(|b| b == b'_') {
                report(&c::NO_LEADING_UNDERSCORES_FOR_LOCAL_IDENTIFIERS, n.span, &[text]);
            }
        };
        let from_list = |l: &VariableList, check: &mut dyn FnMut(ast::Name)| {
            for v in l.variables.iter() {
                check(v.name);
            }
        };
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => from_list(l, &mut check),
                StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => check(*name),
                StmtKind::Function(f) => {
                    if let Some(n) = a.function(*f).name {
                        check(n);
                    }
                }
                StmtKind::Try { catches, .. } => {
                    for k in catches.iter() {
                        for n in k.exception.iter().chain(k.stack_trace.iter()) {
                            check(*n);
                        }
                    }
                }
                _ => {}
            }
        }
        // `visitDeclaredVariablePattern`.
        for n in super::rules::pattern_declared_variables(a) {
            check(n);
        }
        // `visitForPartsWithDeclarations` and `visitDeclaredIdentifier` of a
        // collection `for` and `for-in`.
        fn from_collection(el: &ast::CollectionElement, out: &mut Vec<ast::Name>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ForInit::Variables(l)) = init {
                        out.extend(l.variables.iter().map(|v| v.name));
                    }
                    from_collection(body, out);
                }
                ast::CollectionElement::ForIn { target, body, .. } => {
                    if let ForInTarget::Declared { name, .. } = target {
                        out.push(*name);
                    }
                    from_collection(body, out);
                }
                ast::CollectionElement::If { then, else_, .. } => {
                    from_collection(then, out);
                    if let Some(x) = else_ {
                        from_collection(x, out);
                    }
                }
                _ => {}
            }
        }
        let mut from_collections = Vec::new();
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                for el in elements.iter() {
                    from_collection(el, &mut from_collections);
                }
            }
        }
        for n in from_collections {
            check(n);
        }
        // The unnamed parameters; the list is abandoned at the first `this.x`
        // or `super.x` (the original's `return`).
        for list in &param_lists {
            for p in list.iter() {
                if p.this_ || p.super_ {
                    break;
                }
                if let (false, Some(n)) = (p.kind == ParameterKind::Named, p.name) {
                    check(n);
                }
            }
        }
    }
    // `prefer_function_declarations_over_variables`.
    if enabled("prefer_function_declarations_over_variables") {
        let mut check = |l: &VariableList, local: bool| {
            for v in l.variables.iter() {
                let Some(init) = v.initializer else { continue };
                if !matches!(a.expr(init).kind, ExprKind::FunctionExpression(_)) {
                    continue;
                }
                // Inside a function body, the unchanged element; outside, the
                // `final` list.
                let reported = if local { sem.is_some_and(|s| !super::is_mutated(s, a, v.name.span.start)) } else { l.final_ };
                if reported {
                    report(&c::PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES, Span { start: v.name.span.start, end: a.expr(init).span.end }, &[]);
                }
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                check(l, false);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind {
                check(l, false);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => check(l, true),
                _ => {}
            }
        }
        // The lists of a collection `for` (inside a function body or an
        // initializer: that of a field or top-level initializer is outside a
        // body).
        let bodies: Vec<Span> = a
            .functions
            .iter()
            .filter_map(|f| match &f.body {
                ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            })
            .chain(a.members.iter().filter_map(|m| match &m.kind {
                MemberKind::Constructor(k) => match &k.body {
                    ast::FunctionBody::Block(s) => Some(a.stmt(*s).span),
                    ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                    _ => None,
                },
                _ => None,
            }))
            .collect();
        fn collection_lists<'x>(el: &'x ast::CollectionElement, out: &mut Vec<&'x VariableList>) {
            match el {
                ast::CollectionElement::For { init, body, .. } => {
                    if let Some(ForInit::Variables(l)) = init {
                        out.push(l);
                    }
                    collection_lists(body, out);
                }
                ast::CollectionElement::ForIn { body, .. } => collection_lists(body, out),
                ast::CollectionElement::If { then, else_, .. } => {
                    collection_lists(then, out);
                    if let Some(x) = else_ {
                        collection_lists(x, out);
                    }
                }
                _ => {}
            }
        }
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                let mut ls = Vec::new();
                for el in elements.iter() {
                    collection_lists(el, &mut ls);
                }
                let local = bodies.iter().any(|c| c.start <= e.span.start && e.span.end <= c.end);
                for l in ls {
                    check(l, local);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn findings(source: &str) -> Vec<(&'static str, String)> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        let mut reports = run(u, &names, &|_| true, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn expressions() {
        assert_eq!(findings("int f(int? a) => a == null ? 0 : a;\n"), vec![("prefer_if_null_operators", "a == null ? 0 : a".to_string())]);
        assert!(findings("int f(int? a, int b) => a == null ? 0 : b;\n").is_empty());
        assert_eq!(
            findings("void f(List<int> l) {\n  l..add(1);\n}\n"),
            vec![("avoid_single_cascade_in_expression_statements", "l..add(1)".to_string())]
        );
    }

    #[test]
    fn names_and_parameters() {
        assert_eq!(findings("void f(int g(int x)) {}\n"), vec![("use_function_type_syntax_for_parameters", "int g(int x)".to_string())]);
        assert_eq!(
            findings("void f(int _a, {int? _b}) {\n  var _c = 1;\n  var __ = 2;\n}\n"),
            vec![
                ("no_leading_underscores_for_local_identifiers", "_a".to_string()),
                ("no_leading_underscores_for_local_identifiers", "_c".to_string()),
            ]
        );
    }

    #[test]
    fn functions_in_variables() {
        assert_eq!(findings("final f = () {};\nvar g = () {};\n"), vec![("prefer_function_declarations_over_variables", "f = () {}".to_string())]);
        // The local needs the unit's semantics (the mutation is by element).
        assert!(findings("void m() {\n  var a = () {};\n  var b = () {};\n  b = () {};\n}\n").is_empty());
    }
}
