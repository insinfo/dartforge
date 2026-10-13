//! The fifth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `sort_constructors_first`,
//! `sort_unnamed_constructors_first`,
//! `always_put_required_named_parameters_first`,
//! `avoid_multiple_declarations_per_line`, `eol_at_end_of_file` and
//! `unnecessary_raw_strings`.
//!
//! Written with the emitters of the SDK's `main` open and then checked
//! against those of 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`), which
//! differ in three points, already followed here: in the two constructor
//! order rules the report goes on the class name (`member.returnType`);
//! `sort_unnamed_constructors_first` only looks at an extension type that has
//! a named primary constructor; and `always_put_required_named_parameters_first`
//! does not exempt `super.x` parameters. `eol_at_end_of_file` has a single
//! code.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, DeclKind, ExprKind, MemberId, MemberKind, ParameterKind, StmtKind, VariableList};
use dartforge_intern::Interner;

/// The literal `lexeme` is a single raw string (`r'…'`, `r"""…"""`): it starts
/// with `r` and the opening quote only appears again at the end.
fn is_single_raw_string(lexeme: &str) -> bool {
    let Some(rest) = lexeme.strip_prefix('r') else { return false };
    let quote = if rest.starts_with("'''") {
        "'''"
    } else if rest.starts_with("\"\"\"") {
        "\"\"\""
    } else if rest.starts_with('\'') {
        "'"
    } else if rest.starts_with('"') {
        "\""
    } else {
        return false;
    };
    let inner = &rest[quote.len()..];
    inner.find(quote).is_some_and(|k| k + quote.len() == inner.len())
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, _sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // The two constructor order rules, in classes, enums and extension
    // types.
    let (first, unnamed_first) = (enabled("sort_constructors_first"), enabled("sort_unnamed_constructors_first"));
    if first || unnamed_first {
        for d in a.decls.iter() {
            let members: &[MemberId] = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            // In an extension type, the unnamed rule only applies with a
            // named primary constructor.
            let unnamed_here = unnamed_first && !matches!(&d.kind, DeclKind::ExtensionType(x) if x.constructor.is_none());
            let (mut other, mut named) = (false, false);
            for id in members {
                let MemberKind::Constructor(k) = &a.member(*id).kind else {
                    other = true;
                    continue;
                };
                // `member.returnType`: the class name.
                let range = k.class_name.span;
                if first && other {
                    report(&c::SORT_CONSTRUCTORS_FIRST, range, &[]);
                }
                match k.name {
                    None if unnamed_here && named => report(&c::SORT_UNNAMED_CONSTRUCTORS_FIRST, range, &[]),
                    None => {}
                    Some(_) => named = true,
                }
            }
        }
    }
    // `always_put_required_named_parameters_first`.
    if enabled("always_put_required_named_parameters_first") {
        fn visit(list: &[ast::Parameter], interner: &Interner, findings: &mut Vec<ast::Name>) {
            let mut seen_optional = false;
            for p in list {
                if let Some(inner) = &p.function_parameters {
                    visit(inner, interner, findings);
                }
                if p.kind != ParameterKind::Named {
                    continue;
                }
                let required = p.required
                    || p.metadata.iter().any(|m| m.arguments.is_none() && m.name.last().is_some_and(|n| interner.resolve(n.sym) == "required"));
                if required {
                    if let (true, Some(n)) = (seen_optional, p.name) {
                        findings.push(n);
                    }
                } else {
                    seen_optional = true;
                }
            }
        }
        let mut findings: Vec<ast::Name> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                visit(ps, interner, &mut findings);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                visit(&k.parameters, interner, &mut findings);
            }
        }
        for n in findings {
            report(&c::ALWAYS_PUT_REQUIRED_NAMED_PARAMETERS_FIRST, n.span, &[]);
        }
    }
    // `avoid_multiple_declarations_per_line`: on the second variable of every
    // list, except that of a `for` initialization.
    if enabled("avoid_multiple_declarations_per_line") {
        let mut check = |l: &VariableList| {
            if let Some(second) = l.variables.get(1) {
                report(&c::AVOID_MULTIPLE_DECLARATIONS_PER_LINE, second.name.span, &[]);
            }
        };
        for d in a.decls.iter() {
            if let DeclKind::Variables(l) = &d.kind {
                check(l);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Field(l) = &m.kind {
                check(l);
            }
        }
        for s in a.stmts.iter() {
            if let StmtKind::Variables(l) = &s.kind {
                check(l);
            }
        }
    }
    // `eol_at_end_of_file`: no line break at the end, or more than one.
    if enabled("eol_at_end_of_file") && !source.is_empty() {
        let too_many = source.ends_with("\n\n") || source.ends_with("\r\r") || source.ends_with("\r\n\r\n");
        let missing = !(source.ends_with('\n') || source.ends_with('\r'));
        if too_many || missing {
            let pos = source.trim_end().len();
            // One character from the end of the content (or the end, if none).
            let end = source[pos..].chars().next().map_or(pos, |x| pos + x.len_utf8());
            report(&c::EOL_AT_END_OF_FILE, Span { start: pos, end: end }, &[]);
        }
    }
    // `unnecessary_raw_strings`: a raw string without `\` or `$`.
    if enabled("unnecessary_raw_strings") {
        for e in a.exprs.iter() {
            let ExprKind::String(_) = &e.kind else { continue };
            let Some(lexeme) = source.get(e.span.start..e.span.end) else { continue };
            if is_single_raw_string(lexeme) && !lexeme.contains('\\') && !lexeme.contains('$') {
                report(&c::UNNECESSARY_RAW_STRINGS, e.span, &[]);
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
        let mut reports = run(u, &names, &|rule| rule != "eol_at_end_of_file", None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| (r.code.nome, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn constructor_order() {
        assert_eq!(
            findings("class A {\n  int x = 0;\n  A.n();\n  A();\n}\n"),
            vec![
                ("sort_constructors_first", "A".to_string()),
                ("sort_constructors_first", "A".to_string()),
                ("sort_unnamed_constructors_first", "A".to_string()),
            ]
        );
        assert!(findings("class A {\n  A();\n  A.n();\n  int x = 0;\n}\n").is_empty());
    }

    #[test]
    fn parameters_and_declarations() {
        assert_eq!(
            findings("void f({int? a, required int b}) {}\n"),
            vec![("always_put_required_named_parameters_first", "b".to_string())]
        );
        assert_eq!(findings("int a = 1, b = 2;\n"), vec![("avoid_multiple_declarations_per_line", "b".to_string())]);
        assert!(findings("void f() {\n  for (var i = 0, j = 1; i < j; i++) {}\n}\n").is_empty());
    }

    #[test]
    fn raw_strings_and_end_of_file() {
        assert_eq!(findings("var s = r'abc';\nvar t = r'a\\b';\n"), vec![("unnecessary_raw_strings", "r'abc'".to_string())]);
        assert!(is_single_raw_string("r'''a'b'''") && !is_single_raw_string("'a'") && !is_single_raw_string("r'a' r'b'"));
        let end = |source: &str| {
            let mut names = Interner::new();
            let p = dartforge_frontend::parser::parse(source, &mut names);
            run(Unidade { ast: &p.ast, unit: &p.unit, fonte: source }, &names, &|rule| rule == "eol_at_end_of_file", None).len()
        };
        assert_eq!((end("var a = 1;"), end("var a = 1;\n"), end("var a = 1;\n\n")), (1, 0, 1));
    }
}
