//! The second batch of lint rules that only look at the tree
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8):
//! `avoid_return_types_on_setters`, `empty_constructor_bodies`,
//! `library_prefixes`, `no_leading_underscores_for_library_prefixes`,
//! `constant_identifier_names`, `prefer_is_not_operator`,
//! `prefer_adjacent_string_concatenation`,
//! `unnecessary_null_in_if_null_operators`, `unnecessary_late`,
//! `empty_statements` and `use_rethrow_when_possible`.
//!
//! Written with the emitters of the SDK's `main` open and then checked
//! against those of 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`,
//! extracted from the tag on 2026-10-05). In 3.6.2 the empty body of a
//! `factory` constructor is also reported and the `_` prefix is not exempt
//! (the wildcard is from 3.7). `constant_identifier_names` also looks at
//! every declaration pattern variable (the original's
//! `visitDeclaredVariablePattern` does not ask whether it is constant) and
//! every `const` list (also a `for`'s); `use_rethrow_when_possible` compares
//! the thrown element with the parameter of the innermost `catch` that
//! contains the `throw` (it needs the unit's semantics).
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::{LintReport, is_lower_camel_case};
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    BinaryOp, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, FunctionKind, MemberKind,
    StmtKind, UnaryOp,
};
use dartforge_intern::Interner;

/// `isValidLibraryPrefix`: `^\$?_*[a-z][_a-z\d]*$`.
pub fn is_valid_prefix(name: &str) -> bool {
    let without_dollar = name.strip_prefix('$').unwrap_or(name);
    let mut chars = without_dollar.trim_start_matches('_').chars();
    chars.next().is_some_and(|p| p.is_ascii_lowercase())
        && chars.all(|x| x == '_' || x.is_ascii_lowercase() || x.is_ascii_digit())
}

/// The position of the word `word` in `source[from..until]` (the last one, whole).
fn word_at(source: &str, from: usize, until: usize, word: &str) -> Option<Span> {
    let slice = source.get(from..until)?;
    let b = slice.as_bytes();
    let is_word_char = |x: u8| x.is_ascii_alphanumeric() || x == b'_' || x == b'$';
    let mut found = None;
    let mut start = 0;
    while let Some(k) = slice[start..].find(word) {
        let i = start + k;
        let before = i == 0 || !is_word_char(b[i - 1]);
        let after = b.get(i + word.len()).is_none_or(|x| !is_word_char(*x));
        if before && after {
            found = Some(Span {
                start: from + i,
                end: from + i + word.len(),
            });
        }
        start = i + word.len();
    }
    found
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.source;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };
    // No comment: only whitespace between the braces of an empty block.
    let empty_block = |s: dartforge_frontend::ast::StmtId| {
        let body = a.stmt(s);
        let no_stmts = matches!(&body.kind, StmtKind::Block(l) if l.is_empty());
        let inner = source
            .get(body.span.start + 1..body.span.end.saturating_sub(1))
            .unwrap_or("x");
        (no_stmts && inner.trim().is_empty()).then_some(body.span)
    };

    // `avoid_return_types_on_setters`.
    if enabled("avoid_return_types_on_setters") {
        for f in a.functions.iter() {
            if let (FunctionKind::Setter, Some(t)) = (f.kind, f.return_type) {
                report(
                    &c::AVOID_RETURN_TYPES_ON_SETTERS,
                    a.types[t.0 as usize].span,
                    &[],
                );
            }
        }
    }
    // `empty_constructor_bodies`: a constructor with a `{}` body (in 3.6.2,
    // also a `factory`).
    if enabled("empty_constructor_bodies") {
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind
                && let FunctionBody::Block(s) = &k.body
                && let Some(span) = empty_block(*s)
            {
                report(&c::EMPTY_CONSTRUCTOR_BODIES, span, &[]);
            }
        }
    }
    // `library_prefixes` and `no_leading_underscores_for_library_prefixes`.
    let (prefixes, no_underscore) = (
        enabled("library_prefixes"),
        enabled("no_leading_underscores_for_library_prefixes"),
    );
    if prefixes || no_underscore {
        for d in u.unit.directives.iter() {
            let DirectiveKind::Import {
                prefix: Some(p), ..
            } = &d.kind
            else {
                continue;
            };
            let text = interner.resolve(p.sym);
            if prefixes && !is_valid_prefix(text) {
                report(&c::LIBRARY_PREFIXES, p.span, &[text]);
            }
            if no_underscore && text.starts_with('_') {
                report(
                    &c::NO_LEADING_UNDERSCORES_FOR_LIBRARY_PREFIXES,
                    p.span,
                    &[text],
                );
            }
        }
    }
    // `constant_identifier_names`: enum constants and `const` variables.
    if enabled("constant_identifier_names") {
        let mut check = |n: dartforge_frontend::ast::Name| {
            let text = interner.resolve(n.sym);
            if !is_lower_camel_case(text) {
                report(&c::CONSTANT_IDENTIFIER_NAMES, n.span, &[text]);
            }
        };
        for d in a.decls.iter().filter(|d| !d.augment) {
            match &d.kind {
                DeclKind::Enum(x) => x.constants.iter().for_each(|k| check(k.name)),
                DeclKind::Variables(l) if l.const_ => {
                    l.variables.iter().for_each(|v| check(v.name))
                }
                _ => {}
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind
                && l.const_
            {
                l.variables.iter().for_each(|v| check(v.name));
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l)
                | StmtKind::For {
                    init: Some(dartforge_frontend::ast::ForInit::Variables(l)),
                    ..
                } if l.const_ => l.variables.iter().for_each(|v| check(v.name)),
                _ => {}
            }
        }
        // The lists of a collection `for`.
        fn from_collection(
            el: &dartforge_frontend::ast::CollectionElement,
            out: &mut Vec<dartforge_frontend::ast::Name>,
        ) {
            use dartforge_frontend::ast::{CollectionElement, ForInit};
            match el {
                CollectionElement::For { init, body, .. } => {
                    if let Some(ForInit::Variables(l)) = init
                        && l.const_
                    {
                        out.extend(l.variables.iter().map(|v| v.name));
                    }
                    from_collection(body, out);
                }
                CollectionElement::ForIn { body, .. } => from_collection(body, out),
                CollectionElement::If { then, else_, .. } => {
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
        from_collections.into_iter().for_each(&mut check);
        // `visitDeclaredVariablePattern`.
        for n in super::rules::pattern_declared_variables(a) {
            check(n);
        }
    }
    // The expression rules.
    let (is_not, adjacent, null_in_if_null) = (
        enabled("prefer_is_not_operator"),
        enabled("prefer_adjacent_string_concatenation"),
        enabled("unnecessary_null_in_if_null_operators"),
    );
    if is_not || adjacent || null_in_if_null {
        for e in a.exprs.iter() {
            match &e.kind {
                // `!(x is T)`.
                ExprKind::Unary {
                    op: UnaryOp::Not,
                    operand,
                } if is_not => {
                    if let ExprKind::Parenthesized(inner) = &a.expr(*operand).kind
                        && matches!(a.expr(*inner).kind, ExprKind::Is { negated: false, .. })
                    {
                        report(&c::PREFER_IS_NOT_OPERATOR, e.span, &[]);
                    }
                }
                // `'a' + 'b'`: at the `+`.
                ExprKind::Binary {
                    op: BinaryOp::Add,
                    left,
                    right,
                } if adjacent => {
                    if matches!(a.expr(*left).kind, ExprKind::String(_))
                        && matches!(a.expr(*right).kind, ExprKind::String(_))
                    {
                        let (from, until) = (a.expr(*left).span.end, a.expr(*right).span.start);
                        if let Some(k) = source.get(from..until).and_then(|t| t.find('+')) {
                            report(
                                &c::PREFER_ADJACENT_STRING_CONCATENATION,
                                Span {
                                    start: from + k,
                                    end: from + k + 1,
                                },
                                &[],
                            );
                        }
                    }
                }
                // `x ?? null` and `null ?? x`.
                ExprKind::Binary {
                    op: BinaryOp::IfNull,
                    left,
                    right,
                } if null_in_if_null => {
                    if matches!(a.expr(*right).kind, ExprKind::Null) {
                        report(
                            &c::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS,
                            a.expr(*right).span,
                            &[],
                        );
                    } else if matches!(a.expr(*left).kind, ExprKind::Null) {
                        report(
                            &c::UNNECESSARY_NULL_IN_IF_NULL_OPERATORS,
                            a.expr(*left).span,
                            &[],
                        );
                    }
                }
                _ => {}
            }
        }
    }
    // `unnecessary_late`: a `late` top-level variable or static field with
    // all variables initialized; at the word `late`.
    if enabled("unnecessary_late") {
        let mut check = |list: &dartforge_frontend::ast::VariableList, start: usize| {
            if !list.late || list.variables.iter().any(|v| v.initializer.is_none()) {
                return;
            }
            let Some(first) = list.variables.first() else {
                return;
            };
            if let Some(span) = word_at(source, start, first.name.span.start, "late") {
                report(&c::UNNECESSARY_LATE, span, &[]);
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                check(l, d.span.start);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind
                && l.static_
            {
                check(l, m.span.start);
            }
        }
    }
    // `empty_statements`: every loose `;`, except the one that gives meaning
    // to a `switch` case made only of empty statements (the last of them).
    if enabled("empty_statements") {
        let mut meaningful: Vec<dartforge_frontend::ast::StmtId> = Vec::new();
        for s in a.stmts.iter() {
            if let StmtKind::Switch { cases, .. } = &s.kind {
                for case in cases.iter() {
                    if let Some(last) = case.body.last()
                        && case
                            .body
                            .iter()
                            .all(|x| matches!(a.stmt(*x).kind, StmtKind::Empty))
                    {
                        meaningful.push(*last);
                    }
                }
            }
        }
        for (i, s) in a.stmts.iter().enumerate() {
            if matches!(s.kind, StmtKind::Empty)
                && s.span.end > s.span.start
                && !meaningful.contains(&dartforge_frontend::ast::StmtId(i as u32))
            {
                report(&c::EMPTY_STATEMENTS, s.span, &[]);
            }
        }
    }
    // `use_rethrow_when_possible`: the statement `throw e;` inside the
    // `catch` that declared `e`.
    if enabled("use_rethrow_when_possible")
        && let Some(sem) = sem
    {
        // The `throw`s that are a whole expression statement.
        let throw_stmts: Vec<ExprId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expression(e) if matches!(a.expr(*e).kind, ExprKind::Throw(_)) => {
                    Some(*e)
                }
                _ => None,
            })
            .collect();
        // The unit's `catch` clauses.
        let clauses: Vec<&dartforge_frontend::ast::CatchClause> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Try { catches, .. } => Some(catches.iter()),
                _ => None,
            })
            .flatten()
            .collect();
        let mut reported: Vec<Span> = Vec::new();
        for &e in &throw_stmts {
            let expr = a.expr(e);
            let ExprKind::Throw(x) = &expr.kind else {
                continue;
            };
            // `canonicalElement` of the thrown identifier.
            if !matches!(a.expr(*x).kind, ExprKind::Identifier(_)) {
                continue;
            }
            let Some(decl) = sem.body.local_declaration(*x) else {
                continue;
            };
            // The innermost `catch` that contains the `throw`.
            let Some(k) = clauses
                .iter()
                .filter(|k| k.span.start <= expr.span.start && expr.span.end <= k.span.end)
                .min_by_key(|k| k.span.end - k.span.start)
            else {
                continue;
            };
            if k.exception.is_some_and(|n| n.span.start == decl) {
                reported.push(expr.span);
            }
        }
        for span in reported {
            report(&c::USE_RETHROW_WHEN_POSSIBLE, span, &[]);
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
        let u = Unit {
            ast: &p.ast,
            unit: &p.unit,
            source,
        };
        let mut reports = run(u, &names, &|_| true, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports
            .into_iter()
            .map(|r| (r.code.name, source[r.span.start..r.span.end].to_string()))
            .collect()
    }

    #[test]
    fn prefixes() {
        assert!(is_valid_prefix("math") && is_valid_prefix("_a_b1") && is_valid_prefix("$x"));
        assert!(!is_valid_prefix("Math") && !is_valid_prefix("1a"));
        assert_eq!(
            findings("import 'a.dart' as Abc;\n"),
            vec![("library_prefixes", "Abc".to_string())]
        );
        assert_eq!(
            findings("import 'a.dart' as _a;\n"),
            vec![(
                "no_leading_underscores_for_library_prefixes",
                "_a".to_string()
            )]
        );
    }

    #[test]
    fn declarations() {
        assert_eq!(
            findings("void set x(int v) {}\n"),
            vec![("avoid_return_types_on_setters", "void".to_string())]
        );
        assert_eq!(
            findings("class A {\n  A() {}\n  factory A.f() => A();\n}\n"),
            vec![("empty_constructor_bodies", "{}".to_string())]
        );
        // In 3.6.2 the `_` prefix is not a wildcard: both rules report it.
        assert_eq!(findings("import 'a.dart' as _;\n").len(), 2);
        assert_eq!(
            findings("const MAX_VALUE = 1;\nenum E { One, two }\n"),
            vec![
                ("constant_identifier_names", "MAX_VALUE".to_string()),
                ("constant_identifier_names", "One".to_string()),
            ]
        );
        assert_eq!(
            findings("late int a = 1;\nlate int b;\n"),
            vec![("unnecessary_late", "late".to_string())]
        );
    }

    #[test]
    fn expressions_and_statements() {
        assert_eq!(
            findings("bool f(Object o) => !(o is int);\n"),
            vec![("prefer_is_not_operator", "!(o is int)".to_string())]
        );
        assert_eq!(
            findings("var s = 'a' + 'b';\n"),
            vec![("prefer_adjacent_string_concatenation", "+".to_string())]
        );
        assert_eq!(
            findings("var s = f() ?? null;\nint? f() => 1;\n"),
            vec![("unnecessary_null_in_if_null_operators", "null".to_string())]
        );
        assert_eq!(
            findings("void f() {\n  ;\n}\n"),
            vec![("empty_statements", ";".to_string())]
        );
        // Without the unit's semantics, the thrown element is not known.
        assert!(
            findings("void f() {\n  try {\n    f();\n  } catch (e) {\n    throw e;\n  }\n}\n")
                .is_empty()
        );
    }
}
