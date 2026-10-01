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
    // Segunda rodada (§7): exaustividade, `could_not_infer`, limites inferidos,
    // alias de `typedef`/`Never?` (C9) e parâmetros opcionais não usados.
    (
        "exaustividade.dart",
        "enum E { a, b, c }\nsealed class S {}\nclass A extends S {}\nclass B extends S { final int x; B(this.x); }\nfinal class C implements S {}\nsealed class T<X> {}\nclass TI extends T<int> {}\nclass TG<X> extends T<X> {}\n\nint f1(E e) => switch (e) { E.a => 0, E.b => 1 };\nint f2(E? e) => switch (e) { E.a => 0, E.b => 1, E.c => 2 };\nint f3(bool b) => switch (b) { true => 0 };\nint f4(S s) => switch (s) { A() => 0, B(x: 1) => 1 };\nint f5(S s) => switch (s) { A() => 0, B() => 1, C() => 2, _ => 3 };\nint f6((bool, E) r) => switch (r) { (true, _) => 0, (false, E.a) => 1 };\nint f7(List<int> l) => switch (l) { [] => 0, [_] => 1 };\nint f8(S s) => switch (s) { A() => 0, A() => 1, B() => 2, C() => 3 };\nvoid f9(E e) {\n  switch (e) {\n    case E.a:\n      break;\n    case E.b:\n    case E.c:\n      break;\n    default:\n  }\n}\nvoid f10(S s) {\n  switch (s) {\n    case A():\n      print(1);\n  }\n}\nint f11(T<int> t) => switch (t) { TI() => 0 };\nint f12(Object o) => switch (o) { int() => 0 };\nint f13(Map<String, int> m) => switch (m) { {'a': 1} => 0 };\nint f14(int? i) => switch (i) { int() => 0 };\nint f15(S s) => switch (s) { A() || B() => 0 };\nint f16(bool? b) => switch (b) { true => 0, false => 1 };\nint f17((int, {String n}) r) => switch (r) { (1, n: 'x') => 0 };\nvoid main() {}\n",
        &[("non_exhaustive_switch_expression", 10, 16, 6), ("non_exhaustive_switch_expression", 11, 17, 6), ("non_exhaustive_switch_expression", 12, 19, 6), ("non_exhaustive_switch_expression", 13, 16, 6), ("unreachable_switch_case", 14, 61, 2), ("non_exhaustive_switch_expression", 15, 24, 6), ("non_exhaustive_switch_expression", 16, 24, 6), ("unreachable_switch_case", 17, 43, 2), ("unreachable_switch_default", 25, 5, 7), ("non_exhaustive_switch_statement", 29, 3, 6), ("non_exhaustive_switch_expression", 34, 22, 6), ("non_exhaustive_switch_expression", 35, 22, 6), ("non_exhaustive_switch_expression", 36, 32, 6), ("non_exhaustive_switch_expression", 37, 20, 6), ("non_exhaustive_switch_expression", 38, 17, 6), ("non_exhaustive_switch_expression", 39, 21, 6), ("non_exhaustive_switch_expression", 40, 33, 6)],
    ),
    (
        "inferencia.dart",
        "class A {}\nclass NotA {}\nclass C<T extends A> { C(T Function() f); }\nNotA myF() => NotA();\nT max2<T extends num>(T a, T b) => a;\nclass Foo<T> { U method<U extends T>(U u) => u; }\nvoid staticErrorIfNotNever<T extends Never>(T t) {}\ntypedef Fcon<X> = Function(X);\nclass CFcon<X extends Fcon<X>> {}\ntypedef F<T> = T Function();\nvoid g(F<int> a) { F<String> b = a; print(b); }\nvoid main() {\n  var x = C(myF);\n  print(x);\n  dynamic d = 1;\n  print(max2(d, 2.0));\n  new Foo<String>().method(42);\n  staticErrorIfNotNever(3);\n  CFcon<Fcon<Never?>>? x4;\n  print(x4);\n}\n",
        &[("invalid_assignment", 11, 34, 1), ("could_not_infer", 13, 11, 1), ("type_argument_not_matching_bounds", 13, 11, 1), ("could_not_infer", 17, 21, 6), ("could_not_infer", 18, 3, 21), ("type_argument_not_matching_bounds", 19, 9, 12)],
    ),
    (
        "parametros.dart",
        "class _A {\n  _A([int? x]);\n  void _m({int? y, int? z}) {}\n  void pub([int? w]) {}\n  static void s([int? q]) {}\n}\nvoid _f([int? a, int? b]) {}\nvoid main() {\n  _f(1);\n  _A()._m(y: 2);\n  _A.s();\n  _A().pub();\n}\n",
        &[("unused_element", 2, 12, 1), ("unused_element", 3, 25, 1), ("unused_element", 5, 23, 1), ("unused_element", 7, 23, 1)],
    ),
    (
        "parametros_usados.dart",
        "sealed class State {\n  final String label;\n  final String? since;\n  const State({required this.label, this.since});\n}\nclass DeprecatedState extends State {\n  final String? replacedBy;\n  const DeprecatedState({super.since, this.replacedBy}) : super(label: 'deprecated');\n}\nclass _P {\n  void _m([int? a]) {}\n}\nf() => _P()._m;\nclass _Q { _Q([int? z]) { print(z); } }\nvoid main() { _Q(); }\n",
        &[],
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
