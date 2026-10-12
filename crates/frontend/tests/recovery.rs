//! Recuperação de erros do parser como no fasta do `dart analyze` 3.6.2
//! (`_fe_analyzer_shared` 76.0.0 e `AstBuilder` do analyzer 6.11.0): cada
//! caso é uma amostra do oráculo (`corpus/diagnosticos`), com o código e o
//! trecho exatos que o analyzer relata.
use dartforge_frontend::parser::{Parsed, parse_com};
use dartforge_frontend::{LanguageVersion, LibraryFeatures};
use dartforge_intern::Interner;

fn analisar_na(versao: (u16, u16), fonte: &str) -> Parsed {
    let mut nomes = Interner::new();
    let f = LibraryFeatures::new(LanguageVersion::new(versao.0, versao.1), &[]);
    parse_com(fonte, &mut nomes, f)
}

/// `(código, texto do trecho)` de cada diagnóstico, em ordem de posição (e
/// de código, no mesmo trecho).
fn erros_na(versao: (u16, u16), fonte: &str) -> Vec<(String, String)> {
    let p = analisar_na(versao, fonte);
    let mut ds: Vec<_> = p
        .diagnostics
        .iter()
        .map(|d| {
            let codigo = d.code.map(|c| c.info().nome.to_string()).unwrap_or_default();
            (d.span.start, d.span.end, codigo, fonte[d.span.start..d.span.end].to_string())
        })
        .collect();
    ds.sort();
    ds.into_iter().map(|(_, _, c, t)| (c, t)).collect()
}

fn erros(fonte: &str) -> Vec<(String, String)> {
    erros_na((3, 6), fonte)
}

fn par(c: &str, t: &str) -> (String, String) {
    (c.to_string(), t.to_string())
}

#[test]
fn modificadores_fora_de_ordem_e_estranhos_em_membro() {
    assert_eq!(
        erros("class C { final late int x = 0; }"),
        vec![par("modifier_out_of_order", "late")]
    );
    assert_eq!(
        erros("class C { late covariant var x; }"),
        vec![par("modifier_out_of_order", "covariant")]
    );
    assert_eq!(
        erros("class C { static external void f(); }"),
        vec![par("modifier_out_of_order", "external")]
    );
    assert_eq!(erros("class C { required int x = 0; }"), vec![par("extraneous_modifier", "required")]);
    assert_eq!(erros("class C { final final int x = 0; }"), vec![par("duplicated_modifier", "final")]);
}

#[test]
fn modificadores_estranhos_no_topo() {
    assert_eq!(erros("static int f() => 1;"), vec![par("extraneous_modifier", "static")]);
    assert_eq!(erros("abstract int x = 0;"), vec![par("extraneous_modifier", "abstract")]);
    assert_eq!(erros("const t() => null;"), vec![par("extraneous_modifier", "const")]);
    assert_eq!(erros("var f() => null;"), vec![par("var_return_type", "var")]);
    assert_eq!(erros("abstract enum E { a }"), vec![par("extraneous_modifier", "abstract")]);
    assert_eq!(erros("const class C {}"), vec![par("const_class", "const")]);
}

#[test]
fn modificadores_de_parametro() {
    assert_eq!(erros("f(x, static int y) {}"), vec![par("extraneous_modifier", "static")]);
    assert_eq!(erros("f(x, {static y}) {}"), vec![par("extraneous_modifier", "static")]);
    assert_eq!(erros("f6(const p1) {}"), vec![par("extraneous_modifier", "const")]);
    assert_eq!(erros("void f(covariant int x) {}"), vec![par("extraneous_modifier", "covariant")]);
    assert_eq!(
        erros("class C { void m({covariant required int? i}) {} }"),
        vec![par("modifier_out_of_order", "required")]
    );
    // Válidos: nenhum diagnóstico.
    assert!(erros("class C { C({required this.x, covariant int y = 0}); int x; }").is_empty());
    assert!(erros("void f({required}) {} void g(int required) {}").is_empty());
}

#[test]
fn ordem_errada_de_modificadores_de_classe_vira_membro() {
    // `final abstract class` é um campo `final` sem nome seguido da classe.
    assert_eq!(
        erros("final abstract class A {}"),
        vec![
            par("expected_token", "abstract"),
            par("extraneous_modifier", "abstract"),
            par("modifier_out_of_order", "abstract"),
            par("missing_identifier", "class"),
        ]
    );
    assert_eq!(
        erros("final final class A {}"),
        vec![par("expected_token", "final"), par("missing_identifier", "final")]
    );
    assert_eq!(erros("sealed mixin M {}"), vec![par("sealed_mixin", "sealed")]);
    assert_eq!(erros("final mixin class M {}"), vec![par("final_mixin_class", "final")]);
    assert_eq!(erros("base enum E { a }"), vec![par("base_enum", "base")]);
    assert_eq!(erros("abstract sealed class S {}"), vec![par("abstract_sealed_class", "sealed")]);
    // Válidos.
    for v in [
        "abstract class A {}",
        "abstract base class A {}",
        "abstract final class A {}",
        "abstract interface class A {}",
        "abstract mixin class A {}",
        "abstract base mixin class A {}",
        "base mixin M {}",
        "sealed class S {}",
        "final class F {}",
        "interface class I {}",
        "mixin class M {}",
        "base mixin class M {}",
    ] {
        assert!(erros(v).is_empty(), "{v}: {:?}", erros(v));
    }
}

#[test]
fn membros_de_extension_e_mixin() {
    assert_eq!(
        erros("extension E on int { late final int v; }"),
        vec![par("extension_declares_instance_field", "v")]
    );
    assert_eq!(
        erros("extension E on int { int get foo; }"),
        vec![par("extension_declares_abstract_member", "foo")]
    );
    assert_eq!(
        erros("extension E on int { int operator +(int x); }"),
        vec![par("extension_declares_abstract_member", "+")]
    );
    assert_eq!(erros("extension E on int { E(); }"), vec![par("extension_declares_constructor", "E")]);
    assert_eq!(erros("mixin M { M(); }"), vec![par("mixin_declares_constructor", "M")]);
    assert!(erros("extension E on int { static int x = 0; external int y; int get z => 0; }").is_empty());
}

#[test]
fn extension_sem_on() {
    assert_eq!(
        erros("extension E {}"),
        vec![par("expected_token", "E"), par("expected_type_name", "{")]
    );
}

#[test]
fn atribuicao_a_alvo_nao_atribuivel() {
    assert_eq!(
        erros_na((2, 19), "f(v) { (v) = 0; }"),
        vec![par("illegal_assignment_to_non_assignable", "(v)"), par("missing_assignable_selector", "(v)")]
    );
    assert_eq!(erros("f(v) { (v)++; }"), vec![par("illegal_assignment_to_non_assignable", "++")]);
    assert_eq!(erros("f(v) { ++(v); }"), vec![par("missing_assignable_selector", ")")]);
    assert_eq!(erros_na((2, 19), "f(x) => ((x)) = 1;"), vec![par("missing_assignable_selector", "((x))")]);
    assert_eq!(
        erros("class C { m() { super?.m(); } }"),
        vec![par("invalid_operator_questionmark_period_for_super", "?.")]
    );
    assert!(erros("f(a, b) { a = 1; a.b = 2; a[0] = 3; a?.b = 4; a++; ++a[0]; }").is_empty());
}

#[test]
fn comparacao_encadeada() {
    assert_eq!(erros("f(a, b, c) => a == b == c;"), vec![par("equality_cannot_be_equality_operand", "==")]);
    assert_eq!(erros("f(a, b, c) => a < b < c;"), vec![par("equality_cannot_be_equality_operand", "<")]);
    assert!(erros("f(a, b, c) => a < b == c;").is_empty());
}

#[test]
fn nomes_e_formas_de_membro() {
    assert_eq!(erros("class A { int z = 0, A = 0; }"), vec![par("member_with_class_name", "A")]);
    assert_eq!(erros("class A { int get A => 0; }"), vec![par("member_with_class_name", "A")]);
    assert_eq!(erros("get f2() => null;"), vec![par("getter_with_parameters", "(")]);
    assert_eq!(erros("class C { B.x(); }"), vec![par("invalid_constructor_name", "B")]);
    assert_eq!(erros("class C { static C() {} }"), vec![par("static_constructor", "static")]);
}

#[test]
fn padroes_por_contexto() {
    assert_eq!(
        erros("f() { var x; [x, final y] = [0, 1]; }"),
        vec![par("pattern_assignment_declares_variable", "y")]
    );
    assert_eq!(
        erros("f() { var (var a, b) = (1, 2); }"),
        vec![par("variable_pattern_keyword_in_declaration_context", "var")]
    );
    assert_eq!(
        erros("f(x) { switch (x) { case var when: } }"),
        vec![par("illegal_pattern_variable_name", "when")]
    );
}

#[test]
fn interpolacao_sem_nome_nao_interrompe() {
    assert_eq!(erros("f() { var x = \"$\"; }"), vec![par("missing_identifier", "\"")]);
    assert_eq!(erros("f(x) { var s = \"a $$x b\"; }"), vec![par("missing_identifier", "$")]);
    // O resto da unidade continua sendo analisado.
    assert_eq!(
        erros("f() { var x = '$ '; }\nstatic g() {}"),
        vec![par("missing_identifier", " "), par("extraneous_modifier", "static")]
    );
}

#[test]
fn propriedade_com_palavra_reservada() {
    assert_eq!(erros("class C { var x; m() { x.this; } }"), vec![par("missing_identifier", "this")]);
}

#[test]
fn construtor_com_argumentos_de_tipo_depois_do_nome() {
    assert_eq!(
        erros("f() => new C<int>.named<int>();"),
        vec![par("constructor_with_type_arguments", "named")]
    );
}

#[test]
fn padrao_de_parametros_com_default() {
    assert_eq!(
        erros("typedef F = void Function({int x = 1});"),
        vec![par("default_value_in_function_type", "=")]
    );
    assert_eq!(erros("void f(int x = 1) {}"), vec![par("named_parameter_outside_group", "=")]);
}

#[test]
fn await_e_yield_como_nome_em_corpo_assincrono() {
    assert_eq!(erros("f() async { var await = 1; }"), vec![par("async_keyword_used_as_identifier", "await")]);
    assert!(erros("f() { var await = 1; }").is_empty());
}
