//! A referência de uma unidade (docs/ANALYZER-ESPECIFICACAO.md, T2): qual
//! analyzer dá o nome e o texto dos diagnósticos do arquivo. Os casos são os
//! de `corpus/especificacao/t2/v`, com as saídas dos dois oráculos vivos.
//! Escrito sem compilar nem executar (2026-10-05).

use dartforge_frontend::features::{LibraryFeatures, Referencia};
use dartforge_frontend::parser::parse_com;
use dartforge_intern::Interner;

/// A referência da unidade numa biblioteca 3.6 e, já na forma dela, cada
/// diagnóstico: código, trecho, mensagem e correção.
fn ler(fonte: &str) -> (Referencia, Vec<(&'static str, String, String, String)>) {
    let mut nomes = Interner::new();
    let lido = parse_com(fonte, &mut nomes, LibraryFeatures::piso());
    let referencia = lido.referencia;
    let mut v: Vec<_> = lido
        .diagnostics
        .into_iter()
        .filter_map(|d| d.na_referencia(referencia))
        .map(|d| {
            let correcao = d.correcao().unwrap_or_default();
            (d.code.map_or("", |c| c.info().nome), fonte[d.span.start..d.span.end].to_string(), d.message, correcao)
        })
        .collect();
    v.sort();
    (referencia, v)
}

/// c01: `null-aware-elements` o 3.6.2 conhece (sem versão de lançamento), e
/// escreve `3.6.0` na correção; o arquivo continua dele.
#[test]
fn elemento_nulo_fica_com_o_3_6() {
    let (referencia, v) = ler("void f(int? a) {\n  var x = [?a];\n  print(x);\n}\n");
    assert_eq!(referencia, Referencia::V3_6);
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!(v[0].0, "experiment_not_enabled");
    assert_eq!(v[0].2, "This requires the 'null-aware-elements' language feature to be enabled.");
    assert!(v[0].3.contains("to 3.6.0 or higher"), "{}", v[0].3);
}

/// c08 e c18: sintaxe que o 3.6.2 não conhece passa o arquivo para o 3.13.4.
#[test]
fn sintaxe_desconhecida_passa_para_o_3_13() {
    let (referencia, v) = ler("enum E { a, b }\n\nvoid f() {\n  E e = .a;\n  print(e);\n}\n");
    assert_eq!(referencia, Referencia::V3_13);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].3.contains("to 3.10.0 or higher"), "{}", v[0].3);

    let (referencia, v) = ler("class A {\n  int _x;\n  A({required this._x});\n}\n");
    assert_eq!(referencia, Referencia::V3_13);
    assert!(v.iter().any(|d| d.0 == "experiment_not_enabled" && d.1.ends_with("_x") && d.3.contains("3.12.0")), "{v:?}");
}

/// c02: o `final` do campo de representação, no texto do 3.6.2.
#[test]
fn modificador_de_representacao_no_3_6() {
    let (referencia, v) = ler("extension type E(final int i) {}\n");
    assert_eq!(referencia, Referencia::V3_6);
    assert_eq!(v, [("representation_field_modifier", "final".to_string(), "Representation fields can't have modifiers.".to_string(), "Try removing the modifier.".to_string())]);

    // Sem tipo, o 3.6.2 relata o tipo que falta no próprio `final` (c23, F).
    let (referencia, v) = ler("extension type F(final i) {}\n");
    assert_eq!(referencia, Referencia::V3_6);
    let v: Vec<(&str, &str)> = v.iter().map(|d| (d.0, d.1.as_str())).collect();
    assert_eq!(v, [("expected_representation_type", "final"), ("representation_field_modifier", "final")]);

    // Com dois campos, o 3.6.2 relata o modificador e a vírgula (c23, G).
    let (_, v) = ler("extension type G(final int i, final int j) {}\n");
    let v: Vec<(&str, &str)> = v.iter().map(|d| (d.0, d.1.as_str())).collect();
    assert_eq!(v, [("multiple_representation_fields", ","), ("representation_field_modifier", "final")]);
}

/// c23 inteiro: o `var` declarante faz do 3.13.4 a referência, e então o
/// modificador sai com o texto novo, o tipo que falta vai para o nome e o
/// modificador de `G` não sai.
#[test]
fn representacao_com_var_no_3_13() {
    let fonte = "extension type E(var int i) {}\n\nextension type F(final i) {}\n\nextension type G(final int i, final int j) {}\n";
    let (referencia, v) = ler(fonte);
    assert_eq!(referencia, Referencia::V3_13);
    let nova = "Representation fields can't have the modifier 'var'.";
    let resumo: Vec<(&str, &str, &str)> = v.iter().map(|d| (d.0, d.1.as_str(), d.2.as_str())).collect();
    assert_eq!(
        resumo,
        [
            ("expected_representation_type", "i", "Expected a representation type."),
            ("experiment_not_enabled", "var", "This requires the 'primary-constructors' language feature to be enabled."),
            ("multiple_representation_fields", ",", "Each extension type should have exactly one representation field."),
            ("representation_field_modifier", "final", nova),
            ("representation_field_modifier", "var", nova),
        ]
    );
    let recurso = v.iter().find(|d| d.0 == "experiment_not_enabled").unwrap();
    assert!(recurso.3.contains("to 3.13 or higher"), "{}", recurso.3);
}

/// Acima da 3.6 a referência é sempre o 3.13.4.
#[test]
fn biblioteca_nova_e_do_3_13() {
    let mut nomes = Interner::new();
    let lido = parse_com("void main() {}\n", &mut nomes, LibraryFeatures::atual());
    assert_eq!(lido.referencia, Referencia::V3_13);
    let lido = parse_com("void main() {}\n", &mut nomes, LibraryFeatures::piso());
    assert_eq!(lido.referencia, Referencia::V3_6);
}

/// `named_function_expression` (docs/ANALYZER-ESPECIFICACAO.md, família E):
/// os exemplos do oráculo vivo 3.6.2, com a posição do nome.
#[test]
fn funcao_literal_com_nome() {
    let nomeadas = |fonte: &str| -> Vec<(usize, usize)> {
        let mut nomes = Interner::new();
        parse_com(fonte, &mut nomes, LibraryFeatures::piso())
            .diagnostics
            .iter()
            .filter(|d| d.code.is_some_and(|c| c.info().nome == "named_function_expression"))
            .map(|d| (d.span.start, d.span.end - d.span.start))
            .collect()
    };
    assert_eq!(nomeadas("f() { var f = void g() {}; }\n"), [(19, 1)]);
    assert_eq!(nomeadas("f() { var f = g() {}; }\n"), [(14, 1)]);
    assert_eq!(nomeadas("f() { var f = int g<T>() => 1; }\n"), [(18, 1)]);
    // Chamada comum, função local e inicializador de construtor: nada.
    assert!(nomeadas("f() { g(); h() {} var x = g(); }\n").is_empty());
    assert!(nomeadas("class A {\n  int x;\n  A() : x = g() {}\n}\nint g() => 0;\n").is_empty());
    // A guarda de um caso de `switch` expressão.
    assert!(nomeadas("int f(int o) => switch (o) { _ when g(o) => 1, _ => 0 };\nbool g(int o) => true;\n").is_empty());
}
