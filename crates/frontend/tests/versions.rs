//! The parser and the language version (`docs/VERSOES-LINGUAGEM.md`): a
//! disabled feature is a diagnostic; where the grammar diverges, the version
//! decides.
use dartforge_frontend::ast::{DeclKind, MemberKind};
use dartforge_frontend::parser::{Parsed, parse_with};
use dartforge_frontend::{LanguageVersion, LibraryFeatures};
use dartforge_intern::Interner;

fn parse_at(version: (u16, u16), source: &str) -> (Parsed, Interner) {
    let mut names = Interner::new();
    let f = LibraryFeatures::new(LanguageVersion::new(version.0, version.1), &[]);
    let p = parse_with(source, &mut names, f);
    (p, names)
}

fn messages(p: &Parsed) -> Vec<String> {
    p.diagnostics.iter().map(|d| d.message.clone()).collect()
}

#[test]
fn private_named_only_with_this_and_in_312() {
    let source = "class P { final int _x; P({required this._x}); }";
    let (p, names) = parse_at((3, 12), source);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[0]).kind else {
        panic!()
    };
    let MemberKind::Constructor(k) = &p.ast.member(c.members[1]).kind else {
        panic!()
    };
    let pair = &k.parameters[0];
    assert_eq!(names.resolve(pair.name.unwrap().sym), "_x");
    assert_eq!(names.resolve(pair.external_name().unwrap().sym), "x");

    let (p, _) = parse_at((3, 11), source);
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0].message.contains(
            "This requires the 'private-named-parameters' language feature to be enabled."
        )
    );

    for (source, excerpt) in [
        (
            "void f({int _x = 0}) {}",
            "Named parameters can't start with an underscore.",
        ),
        (
            "class A { A({super._b}); }",
            "Named parameters can't start with an underscore.",
        ),
        (
            "class C { int __x; C({required this.__x}); }",
            "has no corresponding public name",
        ),
        (
            "class C { int _if; C({required this._if}); }",
            "has no corresponding public name",
        ),
        (
            "class C { int _x; C(int x, {required this._x}); }",
            "is already defined",
        ),
    ] {
        let (p, _) = parse_at((3, 13), source);
        assert!(
            messages(&p).iter().any(|m| m.contains(excerpt)),
            "{source}: {:?}",
            messages(&p)
        );
    }
}

#[test]
fn final_and_var_in_plain_parameter_only_up_to_312() {
    let source = "void f(final int x, var y) {} var g = (final z) => z;";
    let (p, _) = parse_at((3, 12), source);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    let (p, _) = parse_at((3, 13), source);
    assert_eq!(p.diagnostics.len(), 3, "{:?}", messages(&p));
    assert!(
        p.diagnostics[0]
            .message
            .contains("Can't have modifier 'final' here.")
    );
    // A function type has no declaring parameter, nor that error.
    let (p, _) = parse_at((3, 13), "typedef F = void Function(int x);");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
}

#[test]
fn dot_shorthand_in_310() {
    use dartforge_frontend::ast::{ExprKind, StmtKind};
    let source = "void main() { Cor c = .vermelho; var p = const .new(1); int i = .parse('4').abs(); switch (c) { case .azul: break; } }";
    let (p, names) = parse_at((3, 10), source);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    let shorthands: Vec<(String, bool)> = p
        .ast
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::DotShorthand { name, const_ } => {
                Some((names.resolve(name.sym).to_string(), *const_))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        shorthands,
        vec![
            ("vermelho".into(), false),
            ("new".into(), true),
            ("parse".into(), false),
            ("azul".into(), false)
        ]
    );
    // The `.parse('4').abs()` chain has the shorthand at its root.
    let init = p.ast.stmts.iter().find_map(|s| match &s.kind {
        StmtKind::Variables(v) if names.resolve(v.variables[0].name.sym) == "i" => {
            v.variables[0].initializer
        }
        _ => None,
    });
    let root = p.ast.shorthand_root(init.unwrap()).unwrap();
    assert!(matches!(
        p.ast.expr(root).kind,
        ExprKind::DotShorthand { .. }
    ));

    let (p, _) = parse_at((3, 9), "void main() { Cor c = .vermelho; }");
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0]
            .message
            .contains("This requires the 'dot-shorthands' language feature to be enabled.")
    );
    let (p, _) = parse_at((3, 10), "var x = const .zero;");
    assert!(
        messages(&p)
            .iter()
            .any(|m| m.contains("Expected to find '('.")),
        "{:?}",
        messages(&p)
    );
}

/// The members of an elaborated class, as text: `field final int x`,
/// `ctor name(params) inits body`.
fn members_(p: &Parsed, names: &Interner, i: usize) -> Vec<String> {
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[i]).kind else {
        panic!("not a class")
    };
    c.members
        .iter()
        .map(|m| match &p.ast.member(*m).kind {
            MemberKind::Field(v) => format!(
                "field{}{}{} {}",
                if v.final_ { " final" } else { "" },
                if v.covariant { " covariant" } else { "" },
                if v.ty.is_some() { " T" } else { "" },
                names.resolve(v.variables[0].name.sym)
            ),
            MemberKind::Constructor(k) => {
                let ps: Vec<String> = k
                    .parameters
                    .iter()
                    .map(|q| {
                        format!(
                            "{}{}",
                            if q.this_ { "this." } else { "" },
                            names.resolve(q.name.unwrap().sym)
                        )
                    })
                    .collect();
                format!(
                    "ctor{}{} {}({}) inits={}",
                    if k.const_ { " const" } else { "" },
                    if k.factory { " factory" } else { "" },
                    k.name
                        .map(|n| names.resolve(n.sym).to_string())
                        .unwrap_or_default(),
                    ps.join(", "),
                    k.initializers.len()
                )
            }
            MemberKind::Method(f) => format!(
                "method {}",
                names.resolve(p.ast.function(*f).name.unwrap().sym)
            ),
        })
        .collect()
}

#[test]
fn primary_constructor_is_elaborated_in_tree() {
    let source = "class P(var int x, final int y, String cru, [covariant var z = 1]) {\n\
                 final up = cru;\n\
                 this : assert(x > 0) { print(cru); }\n\
                 }\n\
                 class const K.c(final int v);\n\
                 class Vazia;";
    let (p, names) = parse_at((3, 13), source);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(
        members_(&p, &names, 0),
        vec![
            "field T x",
            "field final T y",
            "field covariant T z",
            "field final up",
            "ctor (this.x, this.y, cru, this.z) inits=1"
        ]
    );
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[0]).kind else {
        panic!()
    };
    assert_eq!(c.primary_constructor, Some(c.members[4]));
    assert_eq!(
        members_(&p, &names, 1),
        vec!["field final T v", "ctor const c(this.v) inits=0"]
    );
    assert!(members_(&p, &names, 2).is_empty());

    let (p, _) = parse_at((3, 12), "class P(var int x);");
    assert!(
        messages(&p).iter().any(|m| m
            .contains("This requires the 'primary-constructors' language feature to be enabled.")),
        "{:?}",
        messages(&p)
    );
}

#[test]
fn new_and_factory_without_class_name() {
    let source = "class Q { int a; new(this.a); new zero() : a = 0; factory um() => Q(1); factory Q.dois() => Q(2); factory() => Q(3); }";
    let (p, names) = parse_at((3, 13), source);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(
        members_(&p, &names, 0),
        vec![
            "field T a",
            "ctor (this.a) inits=0",
            "ctor zero() inits=1",
            "ctor factory um() inits=0",
            "ctor factory dois() inits=0",
            "ctor factory () inits=0"
        ]
    );
    // Before 3.13, `factory()` is a method.
    let (p, names) = parse_at((3, 12), "class F { int factory() => 1; }");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(members_(&p, &names, 0), vec!["method factory"]);
    // `factory C(` with the class name continues the unnamed constructor.
    let (p, names) = parse_at((3, 13), "class C { factory C() => C._(); C._(); }");
    assert_eq!(members_(&p, &names, 0)[0], "ctor factory () inits=0");
}

#[test]
fn primary_constructor_errors() {
    for (source, excerpt) in [
        // Official analyzer 3.13.4 messages (recorded oracle).
        (
            "class C { this {} }",
            "A primary constructor body requires a primary constructor declaration.",
        ),
        (
            "class C(int x) { this {} this {} }",
            "Only one primary constructor body declaration is allowed.",
        ),
        (
            "class C(int x) { C.outro(); }",
            "can't have non-redirecting generative constructors",
        ),
        (
            "class const C(final int x) { this { } }",
            "The body part of a constant primary constructor can't have a block body.",
        ),
        (
            "class C(int x) { this => 1; }",
            "A primary constructor body can't use '=>'.",
        ),
        (
            "class C(covariant int x);",
            "Can't have modifier 'covariant' in a primary constructor.",
        ),
        (
            "extension type E(var int x) {}",
            "Representation fields can't have modifiers.",
        ),
    ] {
        let (p, _) = parse_at((3, 13), source);
        assert!(
            messages(&p).iter().any(|m| m.contains(excerpt)),
            "{source}: {:?}",
            messages(&p)
        );
    }
}

#[test]
fn null_aware_only_in_38() {
    let (p, _) = parse_at((3, 7), "var l = [?a, 'k': ?b];");
    assert_eq!(p.diagnostics.len(), 2, "{:?}", messages(&p));
    let (p, _) = parse_at((3, 8), "var l = {?a: 1, 'k': ?b, ?c: ?d};");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
}
