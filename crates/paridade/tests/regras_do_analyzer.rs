//! Casos mínimos das regras portadas na rodada de paridade de 2026-09-30
//! (docs/ANALISADOR-PARIDADE-PLANO.md). Cada caso foi conferido contra o
//! `dart analyze` 3.6.2 com `examples/sonda_arquivos.rs --livre`: o
//! esperado abaixo é o que o oráculo relatou (código, linha, coluna,
//! comprimento), e nada além disso pode sair.

use dartforge_paridade::analise::Motor;
use dartforge_paridade::corpus::{self, Grupo};
use dartforge_paridade::oraculo::SdkOraculo;
use dartforge_paridade::{Execucao, filtros, rodar_nosso};
use std::path::PathBuf;

const CASOS: &[(&str, &str, &[(&str, usize, usize, usize)])] = &[
    (
        "dup.dart",
        "class A {\n  static void foo() {}\n}\nclass A {\n  void foo() {}\n}\n",
        &[("conflicting_static_and_instance", 2, 15, 3), ("duplicate_definition", 4, 7, 1)],
    ),
    (
        "tipos.dart",
        "void f(Object x) {\n  if (x is Undef) {}\n  List<Undef2> l = [];\n  print(l);\n  try {} on Undef3 catch (_) {}\n  x as Undef4;\n}\n",
        &[
            ("type_test_with_undefined_name", 2, 12, 5),
            ("non_type_as_type_argument", 3, 8, 6),
            ("non_type_in_catch_clause", 5, 13, 6),
            ("cast_to_non_type", 6, 8, 6),
        ],
    ),
    (
        "fluxo.dart",
        "void f(int x, int? y) {\n  late final int a;\n  print(a);\n  final int b;\n  if (y == null) b = 1;\n  print(b);\n  print(x ?? 0);\n  print(x == null);\n  print(true ? 1 : 2);\n}\n",
        &[
            ("definitely_unassigned_late_local_variable", 3, 9, 1),
            ("read_potentially_unassigned_final", 6, 9, 1),
            ("dead_null_aware_expression", 7, 14, 1),
            ("unnecessary_null_comparison", 8, 11, 7),
            ("dead_code", 9, 20, 1),
        ],
    ),
    (
        "ext.dart",
        "class A {}\nextension E1 on A { int get p => 0; }\nextension E2 on A { int get p => 0; }\nint f(A a) => a.p;\n",
        &[("ambiguous_extension_member_access", 4, 17, 1)],
    ),
    (
        "anot.dart",
        "class A {\n  const A.named();\n}\n@A\nvoid f() {}\n@Undef\nvoid g() {}\n",
        &[("invalid_annotation", 4, 1, 2), ("undefined_annotation", 6, 1, 6)],
    ),
    (
        "padrao.dart",
        "final class A {}\nclass R {}\nvoid f(A x, int y) {\n  if (x case R _) {}\n  if (y case <int>[]) {}\n}\n",
        &[("pattern_never_matches_value_type", 4, 14, 1), ("pattern_never_matches_value_type", 5, 14, 7)],
    ),
    (
        "sup.dart",
        "abstract class A {\n  void foo();\n}\nclass B extends A {\n  void foo() => super.foo();\n  void bar() => super.baz();\n}\n",
        &[("abstract_super_member_reference", 5, 23, 3), ("undefined_super_member", 6, 23, 3)],
    ),
];

#[test]
fn regras_portadas_dao_o_que_o_analyzer_3_6_2_da() {
    let Ok(motor) = Motor::descobrir() else {
        eprintln!("SDK ausente: teste pulado");
        return;
    };
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/tmp-agent/regras-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    std::fs::write(raiz.join("pubspec.yaml"), "name: regras\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    let g = Grupo { sdk: SdkOraculo::V362, pacotes: Vec::new(), origem: String::new() };
    let cfg = corpus::preparar(&raiz, "regras", &g).unwrap();
    let mut fontes = Vec::new();
    for (nome, texto, _) in CASOS {
        std::fs::write(raiz.join(nome), texto).unwrap();
        fontes.push(raiz.join(nome));
    }
    let ex = Execucao {
        trabalhadores: 1,
        tamanho_lote: fontes.len(),
        progresso: false,
        isolar: None,
        tempo_max: std::time::Duration::from_secs(600),
    };
    let r = rodar_nosso(&motor, &raiz, &fontes, Some(&cfg), &filtros::Opcoes::default(), &ex);
    let _ = std::fs::remove_dir_all(&raiz);
    for (nome, _, esperado) in CASOS {
        let mut achado: Vec<(String, usize, usize, usize)> = r
            .registros
            .iter()
            .filter(|x| x.arquivo == *nome)
            .map(|x| (x.code.clone(), x.line, x.column, x.length))
            .collect();
        achado.sort();
        let mut esp: Vec<(String, usize, usize, usize)> = esperado.iter().map(|&(c, l, k, n)| (c.to_string(), l, k, n)).collect();
        esp.sort();
        assert_eq!(achado, esp, "{nome}");
    }
}
