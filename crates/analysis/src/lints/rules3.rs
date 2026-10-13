//! The third batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8): `unnecessary_new`,
//! `prefer_typing_uninitialized_variables`, `always_declare_return_types`,
//! `library_names`, `slash_for_doc_comments` and
//! `unnecessary_brace_in_string_interps`.
//!
//! Written with the emitters of the SDK's `main` open and then checked
//! against those of 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter`,
//! extracted from the tag on 2026-10-05). None of the six differs in the rule.
//! Known differences:
//! `always_declare_return_types` exempts the `test_*`/`solo_test_*` methods
//! when the unit that defines the library is in the package's `test/` (the
//! folder of the nearest `pubspec.yaml`; it needs the unit's semantics);
//! `slash_for_doc_comments` finds each declaration's documentation comment
//! like the parser's `_findComment` (`findDartDoc` after the metadata, and
//! otherwise before each annotation, from the last to the first), with the
//! comments scanned from the source (`dartforge_frontend::comentarios`), on
//! the declarations the original visits (not the extension type), and the
//! first comment before a local function.
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    CreationKeyword, DeclKind, DirectiveKind, ExprKind, ForInit, FunctionId, FunctionKind, MemberKind, StmtKind, StringPart, TypedefKind,
    VariableList,
};
use dartforge_intern::Interner;

/// `isLowerCaseUnderScoreWithDots`: `^_?[_a-z\d]*(?:\.[a-z][_a-z\d]*)*$`.
pub fn is_library_name(name: &str) -> bool {
    let inner_char = |x: char| x == '_' || x.is_ascii_lowercase() || x.is_ascii_digit();
    let mut parts = name.split('.');
    let first = parts.next().unwrap_or("");
    first.chars().all(inner_char) && parts.all(|p| p.chars().next().is_some_and(|x| x.is_ascii_lowercase()) && p.chars().all(inner_char))
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.fonte;
    let mut report = |code: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `unnecessary_new`: at the word `new`.
    if enabled("unnecessary_new") {
        for e in a.exprs.iter() {
            if matches!(e.kind, ExprKind::InstanceCreation { keyword: Some(CreationKeyword::New), .. }) && source.get(e.span.start..).is_some_and(|t| t.starts_with("new")) {
                report(&c::UNNECESSARY_NEW, Span { start: e.span.start, end: e.span.start + 3 }, &[]);
            }
        }
    }
    // `prefer_typing_uninitialized_variables`: an untyped list with a
    // variable without an initializer.
    if enabled("prefer_typing_uninitialized_variables") {
        let mut check = |list: &VariableList, of_field: bool| {
            if list.ty.is_some() {
                return;
            }
            let code =
                if of_field { &c::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_FIELD } else { &c::PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_LOCAL_VARIABLE };
            for v in list.variables.iter().filter(|v| v.initializer.is_none()) {
                report(code, v.name.span, &[]);
            }
        };
        for d in a.decls.iter().filter(|d| !d.augment) {
            if let DeclKind::Variables(l) = &d.kind {
                check(l, false);
            }
        }
        for m in a.members.iter().filter(|m| !m.augment) {
            if let MemberKind::Field(l) = &m.kind {
                check(l, true);
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => check(l, false),
                _ => {}
            }
        }
    }
    // `always_declare_return_types`.
    if enabled("always_declare_return_types") {
        let method_fns: Vec<FunctionId> = a
            .members
            .iter()
            .filter_map(|m| match &m.kind {
                MemberKind::Method(f) => Some(*f),
                _ => None,
            })
            .collect();
        let augmented: Vec<FunctionId> = a
            .members
            .iter()
            .filter(|m| m.augment)
            .filter_map(|m| match &m.kind {
                MemberKind::Method(f) => Some(*f),
                _ => None,
            })
            .chain(a.decls.iter().filter(|d| d.augment).filter_map(|d| match &d.kind {
                DeclKind::Function(f) => Some(*f),
                _ => None,
            }))
            .collect();
        for (i, f) in a.functions.iter().enumerate() {
            let id = FunctionId(i as u32);
            let Some(name) = f.name else { continue };
            if f.return_type.is_some() || f.kind == FunctionKind::Setter || augmented.contains(&id) {
                continue;
            }
            let text = interner.resolve(name.sym);
            if method_fns.contains(&id) {
                let is_test = (text.starts_with("test_") || text.starts_with("solo_test_")) && sem.is_some_and(super::in_package_test);
                if text != "[]=" && !is_test {
                    report(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_METHODS, name.span, &[text]);
                }
            } else {
                report(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS, name.span, &[text]);
            }
        }
        // The old-style `typedef` without a return type.
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && matches!(x.kind, TypedefKind::Legacy { return_type: None, .. })
            {
                report(&c::ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS, x.name.span, &[interner.resolve(x.name.sym)]);
            }
        }
    }
    // `library_names`.
    if enabled("library_names") {
        for d in u.unit.directives.iter() {
            if let DirectiveKind::Library { name } = &d.kind
                && let (Some(p), Some(last)) = (name.first(), name.last())
            {
                let text = name.iter().map(|n| interner.resolve(n.sym)).collect::<Vec<_>>().join(".");
                if !is_library_name(&text) {
                    report(&c::LIBRARY_NAMES, Span { start: p.span.start, end: last.span.end }, &[text.as_str()]);
                }
            }
        }
    }
    // `slash_for_doc_comments`: `/** … */` as documentation.
    if enabled("slash_for_doc_comments") {
        let comments = dartforge_frontend::comentarios::Comentarios::de(source);
        // `_findComment(metadata, tokenAfterMetadata)`.
        let doc = |metadata: &[dartforge_frontend::ast::Annotation], start: usize| {
            let after = match metadata.last() {
                Some(m) => dartforge_frontend::fonte::pular_brancos(source.as_bytes(), m.span.end),
                None => start,
            };
            comments.dart_doc(source, after).or_else(|| metadata.iter().rev().find_map(|m| comments.dart_doc(source, m.span.start)))
        };
        let mut docs: Vec<Span> = Vec::new();
        if let Some(first) = u.unit.directives.first() {
            docs.extend(doc(&first.metadata, first.span.start));
        }
        for d in a.decls.iter() {
            if matches!(d.kind, DeclKind::ExtensionType(_)) {
                continue;
            }
            docs.extend(doc(&d.metadata, d.span.start));
            if let DeclKind::Enum(x) = &d.kind {
                for k in x.constants.iter() {
                    docs.extend(doc(&k.metadata, k.span.start));
                }
            }
        }
        for m in a.members.iter() {
            docs.extend(doc(&m.metadata, m.span.start));
        }
        let mut locals: Vec<Span> = Vec::new();
        for (i, s) in a.stmts.iter().enumerate() {
            if !matches!(s.kind, StmtKind::Function(_)) {
                continue;
            }
            let metadata = a
                .metadados_locais
                .iter()
                .find(|(x, _)| x.0 as usize == i)
                .map(|(_, m)| &m[..])
                .unwrap_or(&[]);
            docs.extend(doc(metadata, s.span.start));
            // `visitFunctionDeclarationStatement`: the first comment before
            // the statement.
            if let Some(first) = comments.antes_de(source, s.span.start).first()
                && source[first.start..first.end].starts_with("/**")
            {
                locals.push(*first);
            }
        }
        // `isJavaStyle`.
        docs.retain(|s| source[s.start..s.end].starts_with("/**"));
        docs.extend(locals);
        docs.sort_by_key(|s| (s.start, s.end));
        docs.dedup();
        for span in docs {
            report(&c::SLASH_FOR_DOC_COMMENTS, span, &[]);
        }
    }
    // `unnecessary_brace_in_string_interps`: `${name}` and `${this}` when
    // what follows would not continue the identifier.
    if enabled("unnecessary_brace_in_string_interps") {
        let b = source.as_bytes();
        for e in a.exprs.iter() {
            let ExprKind::String(lit) = &e.kind else { continue };
            for part in lit.parts.iter() {
                let StringPart::Interpolation(x) = part else { continue };
                let inner = a.expr(*x);
                let simple = match &inner.kind {
                    ExprKind::Identifier(n) => !interner.resolve(n.sym).contains('$'),
                    ExprKind::This => true,
                    _ => false,
                };
                let (start, end) = (inner.span.start, inner.span.end);
                // Written exactly as `${x}`.
                if !simple || start < 2 || b.get(start - 1) != Some(&b'{') || b.get(start - 2) != Some(&b'$') || b.get(end) != Some(&b'}') {
                    continue;
                }
                let continues = b.get(end + 1).is_some_and(|x| x.is_ascii_alphanumeric() || *x == b'_' || *x == b'$');
                if !continues {
                    report(&c::UNNECESSARY_BRACE_IN_STRING_INTERPS, Span { start: start - 2, end: end + 1 }, &[]);
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
        reports.into_iter().map(|r| (r.code.unico, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn library_names() {
        assert!(is_library_name("a.b_c") && is_library_name("_a") && is_library_name(""));
        assert!(!is_library_name("A.b") && !is_library_name("a.1b"));
        assert_eq!(findings("library My.lib;\n"), vec![("library_names", "My.lib".to_string())]);
    }

    #[test]
    fn declarations() {
        assert_eq!(findings("int g() => 1;\nf() {}\n"), vec![("always_declare_return_types_of_functions", "f".to_string())]);
        assert_eq!(
            findings("class A {\n  var x;\n  m() {}\n  operator []=(int i, int v) {}\n}\n"),
            vec![("prefer_typing_uninitialized_variables_for_field", "x".to_string()), ("always_declare_return_types_of_methods", "m".to_string())]
        );
        assert_eq!(findings("int y = 0;\nvar z;\n"), vec![("prefer_typing_uninitialized_variables_for_local_variable", "z".to_string())]);
        assert_eq!(findings("/** doc */\nclass A {}\n/// ok\nclass B {}\n"), vec![("slash_for_doc_comments", "/** doc */".to_string())]);
    }

    #[test]
    fn expressions() {
        assert_eq!(findings("class A {}\nvar a = new A();\n"), vec![("unnecessary_new", "new".to_string())]);
        assert_eq!(findings("String f(int x) => '${x} and ${x}y and ${x + 1}';\n"), vec![("unnecessary_brace_in_string_interps", "${x}".to_string())]);
    }
}
