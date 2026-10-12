//! O parser e a versão de linguagem (`docs/VERSOES-LINGUAGEM.md`): recurso
//! desligado é diagnóstico; onde a gramática diverge, a versão decide.
use dartforge_frontend::ast::{DeclKind, MemberKind};
use dartforge_frontend::parser::{Parsed, parse_com};
use dartforge_frontend::{LanguageVersion, LibraryFeatures};
use dartforge_intern::Interner;

fn na(versao: (u16, u16), fonte: &str) -> (Parsed, Interner) {
    let mut names = Interner::new();
    let f = LibraryFeatures::new(LanguageVersion::new(versao.0, versao.1), &[]);
    let p = parse_com(fonte, &mut names, f);
    (p, names)
}

fn messages(p: &Parsed) -> Vec<String> {
    p.diagnostics.iter().map(|d| d.message.clone()).collect()
}

#[test]
fn private_named_only_with_this_and_in_312() {
    let fonte = "class P { final int _x; P({required this._x}); }";
    let (p, names) = na((3, 12), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[0]).kind else {
        panic!()
    };
    let MemberKind::Constructor(k) = &p.ast.member(c.members[1]).kind else {
        panic!()
    };
    let pair = &k.parameters[0];
    assert_eq!(names.resolve(pair.name.unwrap().sym), "_x");
    assert_eq!(names.resolve(pair.nome_externo().unwrap().sym), "x");

    let (p, _) = na((3, 11), fonte);
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0]
            .message
            .contains("This requires the 'private-named-parameters' language feature to be enabled.")
    );

    for (fonte, excerpt) in [
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
        let (p, _) = na((3, 13), fonte);
        assert!(
            messages(&p).iter().any(|m| m.contains(excerpt)),
            "{fonte}: {:?}",
            messages(&p)
        );
    }
}

#[test]
fn final_and_var_in_plain_parameter_only_up_to_312() {
    let fonte = "void f(final int x, var y) {} var g = (final z) => z;";
    let (p, _) = na((3, 12), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    let (p, _) = na((3, 13), fonte);
    assert_eq!(p.diagnostics.len(), 3, "{:?}", messages(&p));
    assert!(
        p.diagnostics[0]
            .message
            .contains("Can't have modifier 'final' here.")
    );
    // Tipo de função não tem parâmetro declarante nem esse erro.
    let (p, _) = na((3, 13), "typedef F = void Function(int x);");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
}

#[test]
fn dot_shorthand_in_310() {
    use dartforge_frontend::ast::{ExprKind, StmtKind};
    let fonte = "void main() { Cor c = .vermelho; var p = const .new(1); int i = .parse('4').abs(); switch (c) { case .azul: break; } }";
    let (p, names) = na((3, 10), fonte);
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
    // A cadeia `.parse('4').abs()` tem o atalho na raiz.
    let init = p.ast.stmts.iter().find_map(|s| match &s.kind {
        StmtKind::Variables(v) if names.resolve(v.variables[0].name.sym) == "i" => {
            v.variables[0].initializer
        }
        _ => None,
    });
    let root = p.ast.raiz_de_atalho(init.unwrap()).unwrap();
    assert!(matches!(
        p.ast.expr(root).kind,
        ExprKind::DotShorthand { .. }
    ));

    let (p, _) = na((3, 9), "void main() { Cor c = .vermelho; }");
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0]
            .message
            .contains("This requires the 'dot-shorthands' language feature to be enabled.")
    );
    let (p, _) = na((3, 10), "var x = const .zero;");
    assert!(
        messages(&p).iter().any(|m| m.contains("Expected to find '('.")),
        "{:?}",
        messages(&p)
    );
}

/// Os membros de uma classe elaborada, como texto: `campo final int x`,
/// `ctor nome(params) inits body_node`.
fn members_(p: &Parsed, names: &Interner, i: usize) -> Vec<String> {
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[i]).kind else {
        panic!("não é classe")
    };
    c.members
        .iter()
        .map(|m| match &p.ast.member(*m).kind {
            MemberKind::Field(v) => format!(
                "campo{}{}{} {}",
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
                "metodo {}",
                names.resolve(p.ast.function(*f).name.unwrap().sym)
            ),
        })
        .collect()
}

#[test]
fn primary_constructor_is_elaborated_in_tree() {
    let fonte = "class P(var int x, final int y, String cru, [covariant var z = 1]) {\n\
                 final up = cru;\n\
                 this : assert(x > 0) { print(cru); }\n\
                 }\n\
                 class const K.c(final int v);\n\
                 class Vazia;";
    let (p, names) = na((3, 13), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(
        members_(&p, &names, 0),
        vec![
            "campo T x",
            "campo final T y",
            "campo covariant T z",
            "campo final up",
            "ctor (this.x, this.y, cru, this.z) inits=1"
        ]
    );
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[0]).kind else {
        panic!()
    };
    assert_eq!(c.primary_constructor, Some(c.members[4]));
    assert_eq!(
        members_(&p, &names, 1),
        vec!["campo final T v", "ctor const c(this.v) inits=0"]
    );
    assert!(members_(&p, &names, 2).is_empty());

    let (p, _) = na((3, 12), "class P(var int x);");
    assert!(
        messages(&p)
            .iter()
            .any(|m| m.contains("This requires the 'primary-constructors' language feature to be enabled.")),
        "{:?}",
        messages(&p)
    );
}

#[test]
fn new_and_factory_without_class_name() {
    let fonte = "class Q { int a; new(this.a); new zero() : a = 0; factory um() => Q(1); factory Q.dois() => Q(2); factory() => Q(3); }";
    let (p, names) = na((3, 13), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(
        members_(&p, &names, 0),
        vec![
            "campo T a",
            "ctor (this.a) inits=0",
            "ctor zero() inits=1",
            "ctor factory um() inits=0",
            "ctor factory dois() inits=0",
            "ctor factory () inits=0"
        ]
    );
    // Antes da 3.13, `factory()` é método.
    let (p, names) = na((3, 12), "class F { int factory() => 1; }");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
    assert_eq!(members_(&p, &names, 0), vec!["metodo factory"]);
    // `factory C(` com o nome da classe continua o construtor sem nome.
    let (p, names) = na((3, 13), "class C { factory C() => C._(); C._(); }");
    assert_eq!(members_(&p, &names, 0)[0], "ctor factory () inits=0");
}

#[test]
fn primary_constructor_errors() {
    for (fonte, excerpt) in [
        // Mensagens oficiais do analyzer 3.13.4 (oráculo gravado).
        (
            "class C { this {} }",
            "A primary constructor body requires a primary constructor declaration.",
        ),
        (
            "class C(int x) { this {} this {} }",
            "Only one primary constructor body declaration is allowed.",
        ),
        ("class C(int x) { C.outro(); }", "can't have non-redirecting generative constructors"),
        (
            "class const C(final int x) { this { } }",
            "The body part of a constant primary constructor can't have a block body.",
        ),
        ("class C(int x) { this => 1; }", "A primary constructor body can't use '=>'."),
        (
            "class C(covariant int x);",
            "Can't have modifier 'covariant' in a primary constructor.",
        ),
        (
            "extension type E(var int x) {}",
            "Representation fields can't have modifiers.",
        ),
    ] {
        let (p, _) = na((3, 13), fonte);
        assert!(
            messages(&p).iter().any(|m| m.contains(excerpt)),
            "{fonte}: {:?}",
            messages(&p)
        );
    }
}

#[test]
fn null_aware_so_na_38() {
    let (p, _) = na((3, 7), "var l = [?a, 'k': ?b];");
    assert_eq!(p.diagnostics.len(), 2, "{:?}", messages(&p));
    let (p, _) = na((3, 8), "var l = {?a: 1, 'k': ?b, ?c: ?d};");
    assert!(p.diagnostics.is_empty(), "{:?}", messages(&p));
}
