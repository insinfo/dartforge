//! O parser e a versão de linguagem (`docs/VERSOES-LINGUAGEM.md`): recurso
//! desligado é diagnóstico; onde a gramática diverge, a versão decide.
use dartforge_frontend::ast::{DeclKind, MemberKind};
use dartforge_frontend::parser::{Parsed, parse_com};
use dartforge_frontend::{LanguageVersion, LibraryFeatures};
use dartforge_intern::Interner;

fn na(versao: (u16, u16), fonte: &str) -> (Parsed, Interner) {
    let mut nomes = Interner::new();
    let f = LibraryFeatures::new(LanguageVersion::new(versao.0, versao.1), &[]);
    let p = parse_com(fonte, &mut nomes, f);
    (p, nomes)
}

fn mensagens(p: &Parsed) -> Vec<String> {
    p.diagnostics.iter().map(|d| d.message.clone()).collect()
}

#[test]
fn nomeado_privado_so_com_this_e_na_312() {
    let fonte = "class P { final int _x; P({required this._x}); }";
    let (p, nomes) = na((3, 12), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    let DeclKind::Class(c) = &p.ast.decl(p.unit.declarations[0]).kind else {
        panic!()
    };
    let MemberKind::Constructor(k) = &p.ast.member(c.members[1]).kind else {
        panic!()
    };
    let par = &k.parameters[0];
    assert_eq!(nomes.resolve(par.name.unwrap().sym), "_x");
    assert_eq!(nomes.resolve(par.nome_externo().unwrap().sym), "x");

    let (p, _) = na((3, 11), fonte);
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0]
            .message
            .contains("'private-named-parameters' exige a versão de linguagem 3.12")
    );

    for (fonte, trecho) in [
        (
            "void f({int _x = 0}) {}",
            "não inicializa nem declara campo",
        ),
        (
            "class A { A({super._b}); }",
            "não inicializa nem declara campo",
        ),
        (
            "class C { int __x; C({required this.__x}); }",
            "não tem nome público",
        ),
        (
            "class C { int _if; C({required this._if}); }",
            "não tem nome público",
        ),
        (
            "class C { int _x; C(int x, {required this._x}); }",
            "colide com outro parâmetro",
        ),
    ] {
        let (p, _) = na((3, 13), fonte);
        assert!(
            mensagens(&p).iter().any(|m| m.contains(trecho)),
            "{fonte}: {:?}",
            mensagens(&p)
        );
    }
}

#[test]
fn final_e_var_em_parametro_comum_so_ate_a_312() {
    let fonte = "void f(final int x, var y) {} var g = (final z) => z;";
    let (p, _) = na((3, 12), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    let (p, _) = na((3, 13), fonte);
    assert_eq!(p.diagnostics.len(), 3, "{:?}", mensagens(&p));
    assert!(
        p.diagnostics[0]
            .message
            .contains("o modificador 'final' não é permitido aqui")
    );
    // Tipo de função não tem parâmetro declarante nem esse erro.
    let (p, _) = na((3, 13), "typedef F = void Function(int x);");
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
}

#[test]
fn atalho_de_ponto_na_310() {
    use dartforge_frontend::ast::{ExprKind, StmtKind};
    let fonte = "void main() { Cor c = .vermelho; var p = const .new(1); int i = .parse('4').abs(); switch (c) { case .azul: break; } }";
    let (p, nomes) = na((3, 10), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    let atalhos: Vec<(String, bool)> = p
        .ast
        .exprs
        .iter()
        .filter_map(|e| match &e.kind {
            ExprKind::DotShorthand { name, const_ } => {
                Some((nomes.resolve(name.sym).to_string(), *const_))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        atalhos,
        vec![
            ("vermelho".into(), false),
            ("new".into(), true),
            ("parse".into(), false),
            ("azul".into(), false)
        ]
    );
    // A cadeia `.parse('4').abs()` tem o atalho na raiz.
    let init = p.ast.stmts.iter().find_map(|s| match &s.kind {
        StmtKind::Variables(v) if nomes.resolve(v.variables[0].name.sym) == "i" => {
            v.variables[0].initializer
        }
        _ => None,
    });
    let raiz = p.ast.raiz_de_atalho(init.unwrap()).unwrap();
    assert!(matches!(
        p.ast.expr(raiz).kind,
        ExprKind::DotShorthand { .. }
    ));

    let (p, _) = na((3, 9), "void main() { Cor c = .vermelho; }");
    assert_eq!(p.diagnostics.len(), 1);
    assert!(
        p.diagnostics[0]
            .message
            .contains("'dot-shorthands' exige a versão de linguagem 3.10")
    );
    let (p, _) = na((3, 10), "var x = const .zero;");
    assert!(
        mensagens(&p).iter().any(|m| m.contains("exige argumentos")),
        "{:?}",
        mensagens(&p)
    );
}

/// Os membros de uma classe elaborada, como texto: `campo final int x`,
/// `ctor nome(params) inits corpo`.
fn membros(p: &Parsed, nomes: &Interner, i: usize) -> Vec<String> {
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
                nomes.resolve(v.variables[0].name.sym)
            ),
            MemberKind::Constructor(k) => {
                let ps: Vec<String> = k
                    .parameters
                    .iter()
                    .map(|q| {
                        format!(
                            "{}{}",
                            if q.this_ { "this." } else { "" },
                            nomes.resolve(q.name.unwrap().sym)
                        )
                    })
                    .collect();
                format!(
                    "ctor{}{} {}({}) inits={}",
                    if k.const_ { " const" } else { "" },
                    if k.factory { " factory" } else { "" },
                    k.name
                        .map(|n| nomes.resolve(n.sym).to_string())
                        .unwrap_or_default(),
                    ps.join(", "),
                    k.initializers.len()
                )
            }
            MemberKind::Method(f) => format!(
                "metodo {}",
                nomes.resolve(p.ast.function(*f).name.unwrap().sym)
            ),
        })
        .collect()
}

#[test]
fn construtor_primario_e_elaborado_na_arvore() {
    let fonte = "class P(var int x, final int y, String cru, [covariant var z = 1]) {\n\
                 final up = cru;\n\
                 this : assert(x > 0) { print(cru); }\n\
                 }\n\
                 class const K.c(final int v);\n\
                 class Vazia;";
    let (p, nomes) = na((3, 13), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    assert_eq!(
        membros(&p, &nomes, 0),
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
        membros(&p, &nomes, 1),
        vec!["campo final T v", "ctor const c(this.v) inits=0"]
    );
    assert!(membros(&p, &nomes, 2).is_empty());

    let (p, _) = na((3, 12), "class P(var int x);");
    assert!(
        mensagens(&p)
            .iter()
            .any(|m| m.contains("'primary-constructors' exige a versão de linguagem 3.13")),
        "{:?}",
        mensagens(&p)
    );
}

#[test]
fn new_e_factory_sem_o_nome_da_classe() {
    let fonte = "class Q { int a; new(this.a); new zero() : a = 0; factory um() => Q(1); factory Q.dois() => Q(2); factory() => Q(3); }";
    let (p, nomes) = na((3, 13), fonte);
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    assert_eq!(
        membros(&p, &nomes, 0),
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
    let (p, nomes) = na((3, 12), "class F { int factory() => 1; }");
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
    assert_eq!(membros(&p, &nomes, 0), vec!["metodo factory"]);
    // `factory C(` com o nome da classe continua o construtor sem nome.
    let (p, nomes) = na((3, 13), "class C { factory C() => C._(); C._(); }");
    assert_eq!(membros(&p, &nomes, 0)[0], "ctor factory () inits=0");
}

#[test]
fn erros_do_construtor_primario() {
    for (fonte, trecho) in [
        (
            "class C { this {} }",
            "exige um construtor primário no cabeçalho",
        ),
        (
            "class C(int x) { this {} this {} }",
            "só pode haver uma parte 'this'",
        ),
        ("class C(int x) { C.outro(); }", "têm de redirecionar"),
        (
            "class const C(final int x) { this { } }",
            "constante não pode ter corpo",
        ),
        ("class C(int x) { this => 1; }", "tem de ser um bloco"),
        (
            "class C(covariant int x);",
            "'covariant' num parâmetro de construtor primário exige 'var'",
        ),
        (
            "extension type E(var int x) {}",
            "'var' não é permitido na representação",
        ),
    ] {
        let (p, _) = na((3, 13), fonte);
        assert!(
            mensagens(&p).iter().any(|m| m.contains(trecho)),
            "{fonte}: {:?}",
            mensagens(&p)
        );
    }
}

#[test]
fn null_aware_so_na_38() {
    let (p, _) = na((3, 7), "var l = [?a, 'k': ?b];");
    assert_eq!(p.diagnostics.len(), 2, "{:?}", mensagens(&p));
    let (p, _) = na((3, 8), "var l = {?a: 1, 'k': ?b, ?c: ?d};");
    assert!(p.diagnostics.is_empty(), "{:?}", mensagens(&p));
}
