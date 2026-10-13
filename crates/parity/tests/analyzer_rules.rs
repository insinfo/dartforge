//! Minimal cases of the rules ported in the 2026-09-30 parity round
//! (docs/ANALISADOR-PARIDADE-PLANO.md). Each case was checked against
//! `dart analyze` 3.6.2 with `examples/probe_files.rs --livre`: the
//! expectation below is what the oracle reported (code, line, column,
//! length), and nothing beyond that may come out.

use dartforge_parity::analysis::Engine;
use dartforge_parity::corpus::{self, Group};
use dartforge_parity::oracle::OracleSdk;
use dartforge_parity::{Execution, filters, run_ours};
use std::path::PathBuf;

const CASES: &[(&str, &str, &[(&str, usize, usize, usize)])] = &[
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
    // Second round (§7): exhaustiveness, `could_not_infer`, inferred bounds,
    // `typedef`/`Never?` aliases (C9) and unused optional parameters.
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
    // 2026-10-12 round (macOS): modifiers outside `MethodDeclaration`, a bare
    // name in a refutable pattern, annotation through a `typedef` alias in FFI,
    // directive annotations and `late` fields without a false cycle.
    (
        "metodo_modificador.dart",
        "class C {\n  const m1();\n  final m2();\n  abstract void m3();\n  covariant void m4();\n  var get m5;\n  late void m6();\n  required void m7();\n}\n",
        &[("const_method", 2, 3, 5), ("concrete_class_with_abstract_member", 2, 9, 5), ("extraneous_modifier", 3, 3, 5), ("concrete_class_with_abstract_member", 3, 9, 5), ("abstract_class_member", 4, 3, 8), ("concrete_class_with_abstract_member", 4, 12, 10), ("covariant_member", 5, 3, 9), ("concrete_class_with_abstract_member", 5, 13, 10), ("var_return_type", 6, 3, 3), ("concrete_class_with_abstract_member", 6, 7, 7), ("extraneous_modifier", 7, 3, 4), ("concrete_class_with_abstract_member", 7, 8, 10), ("extraneous_modifier", 8, 3, 8), ("concrete_class_with_abstract_member", 8, 12, 10)],
    ),
    (
        "padrao_privado.dart",
        "const _a = 10;\nconst _b = 92;\nconst _c = 93;\nconst _d = 94;\nvoid f(int c) {\n  switch (c) {\n    case _a:\n      print('a');\n  }\n  var x = switch (c) { _b => 1, _ => 2 };\n  if (c case _c) print(x);\n}\nvoid g(int c) {\n  for (var i = 0; i < 2; ++i) {\n    switch (c) {\n      case _d:\n        print('a');\n    }\n  }\n}\nclass K {\n  static const _s = 1;\n  static const _t = 2;\n  void m(int c) {\n    switch (c) { case _s: print(1); }\n  }\n}\nextension E on int {\n  static const _u = 3;\n  void n(int c) { if (c case _u) print(1); }\n}\n",
        &[("unused_field", 23, 16, 2)],
    ),
    (
        "ffi_alias.dart",
        "import 'dart:ffi';\ntypedef DWORD = Uint32;\ntypedef P = Packed;\nfinal class F extends Struct {\n  @DWORD()\n  external int x;\n  @Int8()\n  @DWORD()\n  external int y;\n  external int z;\n}\n",
        &[("extra_annotation_on_struct_field", 8, 3, 8), ("missing_annotation_on_struct_field", 10, 12, 3)],
    ),
    (
        "anotacao_diretiva.dart",
        "@A(Duratio(seconds: 180))\n@A(1 + 'a')\n@B()\nlibrary;\n\nclass A { final Object o; const A(this.o); }\n",
        &[("undefined_function", 1, 4, 7), ("const_with_non_constant_argument", 1, 4, 21), ("const_eval_type_num", 2, 4, 7), ("argument_type_not_assignable", 2, 8, 3), ("undefined_annotation", 3, 1, 4)],
    ),
    (
        "anotacao_import.dart",
        "@A(Duration(seconds: 180))\nimport 'dart:typed_data';\n\nclass A { final Object o; const A(this.o); }\nvoid main() { print(Uint8List(1)); }\n",
        &[],
    ),
    (
        "campo_tardio.dart",
        "class M {\n  late final c = f();\n  late final d = c.toString();\n  int f() => 0;\n}\nclass N {\n  late final c = this.g<int>();\n  late final d = c.isEven;\n  T g<T>() => throw 0;\n}\nvoid main() {\n  print(M().d);\n  print(N().d);\n}\n",
        &[],
    ),
    // Corpus positions (2026-10-12): dead code with a rewritten node (record) and
    // a collection `for` whose initialization does not complete, repeated `:var`,
    // `main` with a function parameter, `..complete()` and the flow of `for (x in e)`.
    (
        "campo_padrao.dart",
        "void f(x) {\n  switch (x) {\n    case (foo: 0, :var foo):\n      break;\n  }\n}\n",
        &[("duplicate_pattern_field", 3, 19, 1), ("unused_local_variable", 3, 24, 3)],
    ),
    (
        "cascata_completer.dart",
        "import 'dart:async';\nvoid f(Completer<int> c) {\n  c..complete();\n}\n",
        &[("null_argument_to_non_null_type", 3, 4, 12)],
    ),
    (
        "for_in_fluxo.dart",
        "void use(Object? o) {}\nvoid f(bool b) {\n  if (b) {\n    int x;\n    late int y;\n    for (x in []) {}\n    for (y in []) {}\n    use(x);\n    use(y);\n  }\n  if (b) {\n    int x;\n    late int y;\n    [for (x in []) 0];\n    [for (y in []) 0];\n    use(x);\n    use(y);\n  }\n}\n",
        &[("not_assigned_potentially_non_nullable_local_variable", 8, 9, 1), ("not_assigned_potentially_non_nullable_local_variable", 16, 9, 1)],
    ),
    (
        "main_funcao.dart",
        "void main(void args()) {}\n",
        &[("main_first_positional_parameter_type", 1, 11, 11)],
    ),
    (
        "morto_record.dart",
        "void f(({int x, int y}) p) {\n  true ? p.x : p.y;\n}\nvoid g(({bool b, }) r) {\n  if (true || r.b) {}\n}\nf2() => [for (var (i) = throw 0; true; 1) 0];\n",
        &[("dead_code", 2, 16, 4), ("dead_code", 5, 12, 6), ("unused_local_variable", 7, 20, 1), ("dead_code", 7, 34, 7)],
    ),
    // Corpus messages (2026-10-12): an alias in the extension type's `implements`,
    // `?` in a clause through an alias that is not a class, `p.f<T>` with `p`
    // undefined, a write through a nullable `this` and a wrong type-argument count.
    (
        "clausula_anulavel.dart",
        "typedef X1 = void Function();\ntypedef X2 = (int, String);\ntypedef X3 = Object;\nclass I {}\nextension type A1(int it) implements X1? {}\nextension type A2(int it) implements X2? {}\nextension type A3(int it) implements X3? {}\nextension type A4(int it) implements dynamic {}\nextension type A5(int it) implements Function? {}\nclass C1 implements X1? {}\nclass C2 implements X3? {}\nclass C3 extends X1? {}\nmixin M1 on X1? {}\nclass G<T> implements T? {}\nclass N1 implements Never? {}\nclass D1 implements dynamic? {}\nvar v = 1;\nclass V1 implements v? {}\n",
        &[("extension_type_implements_disallowed_type", 5, 38, 3), ("nullable_type_in_implements_clause", 5, 38, 3), ("extension_type_implements_disallowed_type", 6, 38, 3), ("nullable_type_in_implements_clause", 6, 38, 3), ("nullable_type_in_implements_clause", 7, 38, 3), ("extension_type_implements_disallowed_type", 8, 38, 7), ("extension_type_implements_disallowed_type", 9, 38, 9), ("nullable_type_in_implements_clause", 9, 38, 9), ("implements_non_class", 10, 21, 2), ("nullable_type_in_implements_clause", 10, 21, 3), ("implements_super_class", 11, 21, 3), ("nullable_type_in_implements_clause", 11, 21, 3), ("extends_non_class", 12, 18, 2), ("nullable_type_in_extends_clause", 12, 18, 3), ("mixin_super_class_constraint_non_interface", 13, 13, 2), ("nullable_type_in_on_clause", 13, 13, 3), ("implements_non_class", 14, 23, 1), ("nullable_type_in_implements_clause", 14, 23, 2), ("implements_non_class", 15, 21, 5), ("nullable_type_in_implements_clause", 15, 21, 6), ("implements_non_class", 16, 21, 7), ("unnecessary_question_mark", 16, 28, 1), ("implements_non_class", 18, 21, 1)],
    ),
    (
        "ffi_contagem.dart",
        "import 'dart:ffi';\nimport 'dart:math' as prefix;\nint f(int x) => x;\nvoid main() {\n  Pointer.fromFunction<Int32 Function(Int32), Int8>(f, 0);\n  Pointer.fromFunction<Void Function()>(prefix);\n}\n",
        &[("wrong_number_of_type_arguments_method", 5, 23, 29), ("must_be_a_native_function_type", 5, 24, 21), ("must_be_a_subtype", 6, 41, 6), ("prefix_identifier_not_followed_by_dot", 6, 41, 6)],
    ),
    (
        "implementa_alias.dart",
        "typedef X1 = void Function();\ntypedef X2 = (int, String);\ntypedef X3 = Function;\ntypedef X4 = Null;\ntypedef X5 = dynamic;\ntypedef X6 = Record;\ntypedef X7<T> = List<T> Function();\nextension type A1(int it) implements X1 {}\nextension type A2(int it) implements X2 {}\nextension type A3(int it) implements X3 {}\nextension type A4(int it) implements X4 {}\nextension type A5(int it) implements X5 {}\nextension type A6(int it) implements X6 {}\nextension type A7(int it) implements X7<int> {}\nextension type A8(int it) implements X1? {}\n",
        &[("extension_type_implements_disallowed_type", 8, 38, 2), ("extension_type_implements_disallowed_type", 9, 38, 2), ("extension_type_implements_disallowed_type", 10, 38, 2), ("extension_type_implements_disallowed_type", 11, 38, 2), ("extension_type_implements_disallowed_type", 12, 38, 2), ("extension_type_implements_disallowed_type", 13, 38, 2), ("extension_type_implements_disallowed_type", 14, 38, 7), ("extension_type_implements_disallowed_type", 15, 38, 3), ("nullable_type_in_implements_clause", 15, 38, 3)],
    ),
    (
        "referencia_prefixada.dart",
        "import 'dart:math' as m;\nf() {\n  unresolved.Class<int>.named();\n  unresolved.foo<int>;\n  var x = unresolved.bar<int>();\n  m.max<int>;\n  m.zzz<int>;\n}\nclass C {\n  void g() {\n    other.h<int>;\n  }\n}\nclass B { List<int> Function<T>() get foo => throw 0; }\nclass D extends B {\n  void g() {\n    foo.call<int>;\n    print(foo.call<int>);\n  }\n}\n",
        &[("undefined_identifier", 3, 3, 10), ("undefined_identifier", 4, 3, 10), ("unused_local_variable", 5, 7, 1), ("undefined_identifier", 5, 11, 10), ("undefined_prefixed_name", 7, 5, 3), ("undefined_identifier", 11, 5, 5), ("undefined_identifier", 17, 5, 3), ("undefined_identifier", 18, 11, 3)],
    ),
    (
        "this_anulavel_escrita.dart",
        "class A {\n  int foo = 0;\n  int get g => 0;\n  set s(int _) {}\n  void m() {}\n}\nextension E on A? {\n  void baz() {\n    foo = 0;\n    foo += 1;\n    foo++;\n    s = 1;\n    print(g);\n    m();\n    m;\n    foo ??= 1;\n  }\n}\n",
        &[("unchecked_use_of_nullable_value", 9, 5, 3), ("unchecked_use_of_nullable_value", 10, 5, 3), ("unchecked_use_of_nullable_value", 11, 5, 3), ("unchecked_use_of_nullable_value", 12, 5, 1), ("unchecked_use_of_nullable_value", 13, 11, 1), ("unchecked_use_of_nullable_value", 14, 5, 1), ("unchecked_use_of_nullable_value", 15, 5, 1), ("unchecked_use_of_nullable_value", 16, 5, 3), ("dead_null_aware_expression", 16, 13, 1)],
    ),
    // Corpus FNs (2026-10-12): `Enum` through a supertype, `Fn<int>.foo`,
    // `dart:_wasm`, interpolated `@docImport`, negative hexadecimal and `is Never?`.
    (
        "alias_funcao_acesso.dart",
        "import 'dart:core' as c;\ntypedef Fn<T> = void Function(T);\nvoid bar() {\n  Fn<c.int>.foo;\n  Fn<c.int>.foo = 7;\n  Fn<c.int>.foo += 1;\n  Fn<c.int>.foo();\n  (Fn<c.int>).foo;\n  Fn<c.int>.hashCode;\n}\nextension E on c.Type {\n  c.int get foo => 1;\n  set foo(c.int v) {}\n}\n",
        &[("undefined_getter", 4, 13, 3), ("undefined_setter", 5, 13, 3), ("undefined_getter", 6, 13, 3), ("undefined_method", 7, 13, 3), ("undefined_getter", 9, 13, 8)],
    ),
    (
        "doc_import_interpolado.dart",
        "/// @docImport '${'foo'}.dart';\nlibrary;\n",
        &[("uri_with_interpolation", 1, 16, 15)],
    ),
    (
        "enum_transitivo.dart",
        "mixin M {}\nabstract class A implements Enum {}\nclass B = Object with M implements A;\nclass C implements A {}\nclass D extends C {}\nabstract class E2 implements A {}\nmixin M2 implements A {}\nclass F with M2 {}\n",
        &[("concrete_class_has_enum_superinterface", 3, 7, 1), ("concrete_class_has_enum_superinterface", 4, 7, 1), ("concrete_class_has_enum_superinterface", 5, 7, 1), ("concrete_class_has_enum_superinterface", 8, 7, 1)],
    ),
    (
        "hex_negativo.dart",
        "var a = -0x8000000000000001;\nvar b = -0x8000000000000000;\nvar c = 0xFFFFFFFFFFFFFFFF;\nvar d = -0xFFFFFFFFFFFFFFFF;\nvoid f(Object? y) {\n  if (y is Never?) {}\n}\n",
        &[("integer_literal_out_of_range", 1, 10, 18), ("integer_literal_out_of_range", 4, 10, 18), ("type_check_with_null", 6, 7, 11)],
    ),
    (
        "wasm_interna.dart",
        "import 'dart:_wasm';\n",
        &[("import_internal_library", 1, 8, 12), ("unused_import", 1, 8, 12)],
    ),
    // Corpus FNs (2026-10-12, 2): `for (constant in …)`, `(_) && p`, inherited static
    // `m<T>`, repeated `[super._]` and the scope of each `if`/`while`/`do` branch.
    (
        "escopo_de_ramo.dart",
        "var x = 1;\nvoid f() {\n  while (false) var x = 0;\n  print(x);\n}\nvoid g(bool b) {\n  if (b) {} else var x = 0;\n  print(x);\n  do var x = 0; while (x > 0);\n  print(x);\n}\n",
        &[("dead_code", 3, 17, 10), ("unused_local_variable", 3, 21, 1), ("unused_local_variable", 7, 22, 1), ("unused_local_variable", 9, 10, 1)],
    ),
    (
        "estatico_instanciado.dart",
        "class A {\n  static void a<T>() {}\n  static T g<T>(T x) => x;\n}\nclass B extends A {\n  void b() {\n    a<int>;\n    a;\n    print(g<int>);\n  }\n  static void s() {\n    a<int>;\n  }\n}\n",
        &[("unqualified_reference_to_non_local_static_member", 7, 5, 1), ("unqualified_reference_to_non_local_static_member", 9, 11, 1), ("unqualified_reference_to_non_local_static_member", 12, 5, 1)],
    ),
    (
        "for_in_const.dart",
        "f(Object? o) {\n  const x = 0;\n  for (x in <int>[1, 2]) {\n    print(x);\n  }\n  if (o case (_) && 0) {}\n}\nclass A {\n  final int x, y;\n  A(this.x, [this.y = 0]);\n}\nclass C extends A {\n  final int c;\n  C(this.c, super._, [super._]);\n}\n",
        &[("assignment_to_const", 3, 8, 1), ("unnecessary_wildcard_pattern", 6, 15, 1), ("duplicate_definition", 14, 29, 1), ("missing_default_value_for_parameter", 14, 29, 1)],
    ),
    (
        "super_curinga.dart",
        "class A {\n  final int x, y;\n  A(this.x, [this.y = 0]);\n}\nclass C extends A {\n  C(super.x, [super._]);\n}\nclass D extends A {\n  D(super._, [super.y]);\n}\n",
        &[],
    ),
    // Packages and real projects (2026-10-12): capture only of variables from
    // outside the function, a guarded condition before 2.15, `/x.dart` in a
    // `lib/` library and a private constructor of another library.
    (
        "captura_local.dart",
        "void f(List<int> l) {\n  void g(Map<int, int>? m) {\n    m ??= {};\n    l.forEach((x) {\n      if (!m!.containsKey(x)) print(m.keys);\n    });\n  }\n  Map<int, int>? n;\n  l.forEach((x) {\n    if (n!.containsKey(x)) print(n.keys);\n  });\n  n = {};\n  g(null);\n}\n",
        &[],
    ),
    (
        "condicao_2_14.dart",
        "// @dart=2.14\nvoid f({List<String>? join}) {\n  final bool b = join != null;\n  if (b) print(join!.length);\n  var c = join != null;\n  if (c) print(join!.length);\n}\n",
        &[("unnecessary_non_null_assertion", 4, 20, 1)],
    ),
    (
        "lib/src/a.dart",
        "import '/z.dart';\nimport '/nao.dart';\nvoid f() {}\n",
        &[("unused_import", 1, 8, 9), ("uri_does_not_exist", 2, 8, 11)],
    ),
    (
        "lib/z.dart",
        "class Z {}\n",
        &[],
    ),
    (
        "privado_lib.dart",
        "class Class {\n  Class._constructor();\n  Class();\n}\n",
        &[("unused_element", 2, 9, 12)],
    ),
    (
        "privado_uso.dart",
        "import 'privado_lib.dart';\nimport 'privado_lib.dart' as p;\nmain() {\n  new Class._constructor();\n  new p.Class._constructor();\n  Class();\n}\n",
        &[("new_with_undefined_constructor", 4, 13, 12), ("new_with_undefined_constructor", 5, 15, 12)],
    ),
    // Private static member of another library (2026-10-12).
    (
        "privado_estatico_lib.dart",
        "enum E2 { _A, b }\nclass C { static int _s = 0; static void _m() {} static int get pub => _s; }\n",
        &[("unused_field", 1, 11, 2), ("unused_element", 2, 42, 2)],
    ),
    (
        "privado_estatico_uso.dart",
        "import 'privado_estatico_lib.dart';\nvoid f() {\n  print(E2._A);\n  print(C._s);\n  C._m();\n  C._s = 1;\n  print(C.pub);\n}\nvoid g() {\n  C._s += 1;\n  C._s++;\n}\n",
        &[("undefined_enum_constant", 3, 12, 2), ("undefined_getter", 4, 11, 2), ("undefined_method", 5, 5, 2), ("private_setter", 6, 5, 2), ("private_setter", 10, 5, 2), ("undefined_getter", 10, 5, 2), ("private_setter", 11, 5, 2), ("undefined_getter", 11, 5, 2)],
    ),
    // `f.call` read with `f: Function` and the invocation of `Null`/`Never?` (2026-10-12).
    (
        "call_de_function.dart",
        "void f(Function a, void Function(int) b) {\n  var x = a.call;\n  var y = (a).call;\n  var z = b.call;\n  int i = x;\n  int j = y;\n  int k = z;\n  print([i, j, k]);\n}\nvoid g(Function a) {\n  var r = a.call();\n  int i = r;\n  a.call;\n  print(i);\n}\n",
        &[("invalid_assignment", 5, 11, 1), ("invalid_assignment", 6, 11, 1), ("invalid_assignment", 7, 11, 1)],
    ),
    (
        "invocacao_de_null.dart",
        "void f(Never? x, Null y, void Function()? z) {\n  x();\n  y();\n  z();\n  null();\n  y(1, 2);\n  print(1);\n}\n",
        &[("unchecked_use_of_nullable_value", 2, 3, 1), ("invalid_use_of_null_value", 3, 3, 1), ("unchecked_use_of_nullable_value", 4, 3, 1), ("invalid_use_of_null_value", 5, 3, 4), ("invalid_use_of_null_value", 6, 3, 1)],
    ),
    // `values` and the named primary constructor as members; unnamed `A.new(…)` (2026-10-12).
    (
        "conflito_variavel_de_tipo.dart",
        "enum E<values> { a }\nenum F<a> { a }\nextension type A<T>.T(int it) {}\nextension type B<U>.named(int it) {}\nextension type C<it>(int it) {}\n",
        &[("conflicting_type_variable_and_member", 1, 8, 6), ("conflicting_type_variable_and_member", 2, 8, 1), ("conflicting_type_variable_and_member", 3, 18, 1), ("conflicting_type_variable_and_member", 5, 18, 2)],
    ),
    (
        "primario_new.dart",
        "extension type A.new(int it) {\n  A(this.it);\n}\n",
        &[("duplicate_constructor", 2, 3, 1)],
    ),
    // A synthetic identifier is not an undefined name, so it does not
    // suppress the import warnings (2026-10-13).
    (
        "await_sintetico.dart",
        "import 'dart:async';\nget await => 4;\ntest1() async {\n  var y = await;\n}\n",
        &[("unused_import", 1, 8, 12), ("unused_local_variable", 4, 7, 1), ("missing_identifier", 4, 16, 1)],
    ),
    // `this` in an initializer has the class's type; `this?.m()` is read as
    // `this.<synthetic>?.m()` (2026-10-13).
    (
        "this_no_inicializador.dart",
        "class B {\n  int x = 0;\n  B();\n  B.n();\n  B.a() : this.foo;\n  B.b() : x = this.bar;\n  B.c() : this?.n();\n}\n",
        &[
            ("invalid_reference_to_this", 5, 11, 4),
            ("missing_assignment_in_initializer", 5, 11, 4),
            ("initializer_for_non_existent_field", 5, 11, 8),
            ("undefined_getter", 5, 16, 3),
            ("invalid_reference_to_this", 6, 15, 4),
            ("undefined_getter", 6, 20, 3),
            ("invalid_reference_to_this", 7, 11, 4),
            ("missing_assignment_in_initializer", 7, 11, 4),
            ("expected_token", 7, 15, 2),
            ("initializer_for_non_existent_field", 7, 11, 9),
        ],
    ),
    // Pub-cache FP of `unchecked_use_of_nullable_value` (2026-10-13): `T?`
    // promoted by `is R` to `T & R`; `r?.w(1)()` shorts the invocation too;
    // the assert message sees the condition's false branch.
    (
        "nulos_pub_cache.dart",
        "class R {\n  int get h => 1;\n  int Function() w(int x) => () => x;\n}\nvoid f<T>(T? d, R? r, String? s) {\n  if (d is R) print(d.h);\n  var x = r?.w(1)();\n  int y = r?.w(1)();\n  assert(s == null || s.isEmpty, s.length);\n  assert(true, s.length);\n  print([x, y]);\n}\n",
        &[("invalid_assignment", 8, 11, 9), ("unchecked_use_of_nullable_value", 10, 18, 6)],
    ),
    // `[m]` in a type's comment resolves `m` on the type, through the imported
    // extensions that apply to it (stream_transform; 2026-10-13).
    (
        "doc_ext_s.dart",
        "import 'doc_ext_e.dart';\nextension S<T> on Stream<T> {\n  /// See [expandIt].\n  Stream<T> sw() => this;\n}\n",
        &[],
    ),
    (
        "doc_ext_e.dart",
        "import 'doc_ext_s.dart';\nextension E<T> on Stream<T> {\n  /// See [sw].\n  Stream<T> expandIt() => this;\n}\n",
        &[],
    ),
    (
        "doc_ext_c.dart",
        "import 'doc_ext_e.dart';\nimport 'doc_ext_s.dart' as p;\nabstract class C<T> extends Stream<T> {\n  /// [expandIt] and [sw]\n  void m() {}\n}\n",
        &[],
    ),
    (
        "doc_ext_d.dart",
        "import 'doc_ext_e.dart';\nclass D {\n  /// [expandIt]\n  void m() {}\n}\n",
        &[("unused_import", 1, 8, 16)],
    ),
    // An `if (… case …)` in a case's body does not replace the `switch`'s
    // scrutinee: `case null` keeps promoting it for the later cases (sass;
    // 2026-10-13).
    (
        "caso_aninhado.dart",
        "int? peek() => null;\nvoid f() {\n  switch (peek()) {\n    case null:\n      return;\n    case 1:\n      if (peek() case int(isEven: true) || 3) {\n        print(0);\n      }\n    case >= 3 && <= 9:\n      print(1);\n  }\n  switch (peek()) {\n    case 1:\n      print([if (peek() case 4) 1]);\n    case > 3:\n      print(2);\n  }\n}\n",
        &[("unchecked_use_of_nullable_value", 16, 10, 1)],
    ),
    // `identical(x, null)` of `dart:core` promotes like `x == null`
    // (ngcompiler; 2026-10-13).
    (
        "identical_nulo.dart",
        "import 'dart:core' as core;\nimport 'dart:core';\nclass N { N? next; int v = 0; }\nvoid f(N? n, int? i) {\n  N? c = n;\n  while (!identical(c, null)) {\n    print(c.v);\n    c = c.next;\n  }\n  if (identical(null, i)) return;\n  print(i.isEven);\n  if (core.identical(n, null)) return;\n  print(n.v);\n}\nbool identical2(Object? a, Object? b) => true;\nvoid g(int? i) {\n  if (identical2(i, null)) return;\n  print(i.isEven);\n}\n",
        &[("unchecked_use_of_nullable_value", 18, 11, 6)],
    ),
    // A `const` redirecting factory passes its type arguments to the written
    // target (`Stream.empty() = _EmptyStream<T>`; http, sass, async; 2026-10-13).
    (
        "fabrica_redirecionada.dart",
        "import 'dart:async';\nfinal class BS extends StreamView<List<int>> {\n  const BS(super.stream);\n}\nconst x = BS(Stream.empty());\nconst y = Stream<int>.empty();\nconst z = StreamView<int>(Stream.empty());\n",
        &[],
    ),
    // A `late final` field without an initializer has a setter that a setter
    // with `@override` overrides (code_builder; 2026-10-13).
    (
        "late_final_setter.dart",
        "abstract class A {\n  late final String name;\n  late final int x = 1;\n  final int y = 2;\n}\nclass B extends A {\n  @override\n  set name(String v) {}\n  @override\n  set x(int v) {}\n  @override\n  set y(int v) {}\n}\nvoid f(A a) {\n  a.name = 's';\n}\nabstract class I { late final String q; }\nclass J implements I {\n  @override\n  String get q => '';\n  @override\n  set q(String v) {}\n}\nmixin M { late final int r; }\nclass K with M {\n  @override\n  set r(int v) {}\n}\n",
        &[("override_on_non_overriding_member", 10, 7, 1), ("override_on_non_overriding_member", 12, 7, 1)],
    ),
    // UP of an extension type that implements `Object` and a class is `Object`
    // (limitless_ui with package:web; 2026-10-13).
    (
        "up_tipo_de_extensao.dart",
        "class A {}\nclass B {}\nextension type E(B b) implements Object {}\nextension type G(B b) {}\nvoid f(E? x, A a, G? z) {\n  int i = x ?? a;\n  int l = z ?? a;\n  print([i, l]);\n}\n",
        &[("invalid_assignment", 6, 11, 6), ("invalid_assignment", 7, 11, 6)],
    ),
    // `List<int>? <# List<T>` constrains `T :> int`: the analyzer's gatherer
    // goes on after the `P0?` rule fails, and the interface rule ignores the
    // `?` (2026-10-13).
    (
        "argumento_anulavel_inferencia.dart",
        "class C5 {\n  List<int>? bad;\n  f<T>(List<T> x) {}\n}\nT? g<T>(List<T> x) => null;\nvoid h(C5 c, List<int>? l) {\n  c.f(c.bad);\n  List<int> y = g(l);\n  print(y);\n}\n",
        &[("argument_type_not_assignable", 7, 7, 5), ("invalid_assignment", 8, 17, 4), ("argument_type_not_assignable", 8, 19, 1)],
    ),
    // A plain `=` has no `readElement`: a deprecated getter does not count
    // when only the setter is written (analyzer's resolve_file.dart; 2026-10-13).
    (
        "obsoleto_so_setter.dart",
        "class C {\n  int _o = 0;\n  @Deprecated('use x')\n  int get o => _o;\n  set o(int v) => _o = v;\n  int _p = 0;\n  int get p => _p;\n  @Deprecated('use y')\n  set p(int v) => _p = v;\n}\nvoid f(C c) {\n  c.o = 1;\n  c.o += 1;\n  c.o ??= 2;\n  c.o++;\n  c.p = 1;\n  c.p += 1;\n  print(c.o);\n}\n",
        &[
            ("deprecated_member_use_from_same_package", 13, 5, 1),
            ("deprecated_member_use_from_same_package", 14, 5, 1),
            ("dead_null_aware_expression", 14, 11, 1),
            ("deprecated_member_use_from_same_package", 15, 3, 3),
            ("deprecated_member_use_from_same_package", 16, 5, 1),
            ("deprecated_member_use_from_same_package", 17, 5, 1),
            ("deprecated_member_use_from_same_package", 18, 11, 1),
        ],
    ),
    // A comment ending in `.` before a member is not the `.` of a recovered
    // constructor name (2026-10-13).
    (
        "comentario_antes_do_membro.dart",
        "class A2 {\n  // Intentionally abstract method.\n  method();\n}\nclass B {\n  B();\n  A.make() {}\n  method();\n}\n",
        &[
            ("concrete_class_with_abstract_member", 3, 3, 9),
            ("invalid_constructor_name", 7, 3, 1),
            ("concrete_class_with_abstract_member", 8, 3, 9),
        ],
    ),
    // A compound assignment does not use the operator of a generic extension
    // (`visitAssignmentExpression` keeps the substituted member; 2026-10-13).
    (
        "operador_de_extensao_generica.dart",
        "extension<T> on T Function(T) {\n  T Function(T) operator*(T Function(T) other) {\n    return (value) => this(other(value));\n  }\n}\nextension on int {\n  int operator %(int o) => 0;\n}\nextension<T> on List<T> {\n  List<T> operator -(int o) => this;\n}\nextension<T> on Set<T> {\n  Set<T> operator ~/(int o) => this;\n}\nvoid f() {\n  var g = (int i) => i + 1;\n  g *= (i) => i + 10;\n  print(g(0));\n  var x = 1;\n  x %= 2;\n  var l = [1];\n  l -= 1;\n  l = l - 1;\n  var s = {1};\n  s ~/= 2;\n  print([x, l, s]);\n}\n",
        &[("unused_element", 2, 25, 1), ("unused_element", 7, 16, 1), ("unused_element", 13, 19, 2)],
    ),
    // A reserved word as a local variable's name (2026-10-13).
    (
        "palavra_reservada_local.dart",
        "main() {\n  var enum;\n  var super;\n}\n",
        &[
            ("expected_identifier_but_got_keyword", 2, 7, 4),
            ("unused_local_variable", 2, 7, 4),
            ("expected_identifier_but_got_keyword", 3, 7, 5),
            ("unused_local_variable", 3, 7, 5),
        ],
    ),
];

#[test]
fn ported_rules_give_what_analyzer_3_6_2_gives() {
    let Ok(engine) = Engine::discover() else {
        eprintln!("SDK missing: test skipped");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/tmp-agent/regras-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("pubspec.yaml"), "name: regras\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    let g = Group { sdk: OracleSdk::V362, packages: Vec::new(), origin: String::new() };
    let cfg = corpus::prepare(&root, "regras", &g).unwrap();
    let mut sources = Vec::new();
    for (name, text, _) in CASES {
        std::fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
        std::fs::write(root.join(name), text).unwrap();
        sources.push(root.join(name));
    }
    let ex = Execution {
        workers: 1,
        batch_size: sources.len(),
        progress: false,
        isolate: None,
        max_time: std::time::Duration::from_secs(600),
    };
    let r = run_ours(&engine, &root, &sources, Some(&cfg), &filters::AnalysisOptions::default(), &ex);
    let _ = std::fs::remove_dir_all(&root);
    for (name, _, expected) in CASES {
        let mut found: Vec<(String, usize, usize, usize)> = r
            .records
            .iter()
            .filter(|x| x.file == *name)
            .map(|x| (x.code.clone(), x.line, x.column, x.length))
            .collect();
        found.sort();
        let mut exp: Vec<(String, usize, usize, usize)> = expected.iter().map(|&(c, l, k, n)| (c.to_string(), l, k, n)).collect();
        exp.sort();
        assert_eq!(found, exp, "{name}");
    }
}

/// The root and the files reached through another spelling of the same
/// directory (a link here; a different case on macOS or Windows, where the
/// file system ignores it): the loading canonicalizes the paths, and the
/// reports still come back at the paths as given, with "same package" judged
/// on the canonical root (`deprecated_member_use_from_same_package`, not
/// `deprecated_member_use`).
#[cfg(unix)]
#[test]
fn root_spelled_differently_from_the_file_system() {
    let Ok(engine) = Engine::discover() else {
        eprintln!("SDK missing: test skipped");
        return;
    };
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/tmp-agent/grafia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let real = base.join("real");
    std::fs::create_dir_all(real.join("lib")).unwrap();
    std::fs::create_dir_all(real.join("bin")).unwrap();
    std::fs::write(real.join("pubspec.yaml"), "name: grafia\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    std::fs::write(real.join("lib/a.dart"), "@Deprecated('no')\nvoid old() {}\n").unwrap();
    std::fs::write(real.join("bin/main.dart"), "import 'package:corpus_grafia/a.dart';\nvoid main() => old();\n").unwrap();
    let g = Group { sdk: OracleSdk::V362, packages: Vec::new(), origin: String::new() };
    let cfg = corpus::prepare(&real, "grafia", &g).unwrap();
    let link = base.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let sources = vec![link.join("bin/main.dart")];
    let ex = Execution { workers: 1, batch_size: 1, progress: false, isolate: None, max_time: std::time::Duration::from_secs(600) };
    let cfg_link = link.join(cfg.strip_prefix(&real).unwrap());
    let r = run_ours(&engine, &link, &sources, Some(&cfg_link), &filters::AnalysisOptions::default(), &ex);
    let _ = std::fs::remove_dir_all(&base);
    let found: Vec<(String, String, usize)> = r.records.iter().map(|x| (x.file.clone(), x.code.clone(), x.line)).collect();
    assert_eq!(found, vec![("bin/main.dart".to_string(), "deprecated_member_use_from_same_package".to_string(), 2)]);
}

/// `@internal` is the getter of `package:meta`: a package's own
/// `const internal = …` does not make an export invalid (image; checked
/// against `dart analyze` 3.6.2, 2026-10-13).
#[test]
fn internal_is_the_meta_getter() {
    let Ok(engine) = Engine::discover() else {
        eprintln!("SDK missing: test skipped");
        return;
    };
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repo.join(format!("target/tmp-agent/interno-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib/src")).unwrap();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::write(root.join("pubspec.yaml"), "name: interno\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    let meta = std::fs::canonicalize(repo.join("corpus/diagnosticos/pacotes/meta")).unwrap();
    let cfg = serde_json::json!({ "configVersion": 2, "packages": [
        { "name": "interno", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6" },
        { "name": "meta", "rootUri": format!("file://{}", meta.display()), "packageUri": "lib/", "languageVersion": "2.12" },
    ]});
    std::fs::write(root.join(".dart_tool/package_config.json"), cfg.to_string()).unwrap();
    std::fs::write(root.join("lib/src/i.dart"), "class _I { const _I(); }\nconst internal = _I();\n").unwrap();
    std::fs::write(
        root.join("lib/src/a.dart"),
        "import 'i.dart';\nimport 'package:meta/meta.dart' as meta;\n@internal\nclass A {}\n@meta.internal\nclass B {}\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/p.dart"), "export 'src/a.dart';\n").unwrap();
    let sources = vec![root.join("lib/p.dart")];
    let ex = Execution { workers: 1, batch_size: 1, progress: false, isolate: None, max_time: std::time::Duration::from_secs(600) };
    let r = run_ours(&engine, &root, &sources, Some(&root.join(".dart_tool/package_config.json")), &filters::AnalysisOptions::default(), &ex);
    let _ = std::fs::remove_dir_all(&root);
    let found: Vec<(String, String, usize)> = r.records.iter().map(|x| (x.file.clone(), x.code.clone(), x.line)).collect();
    assert_eq!(found, vec![("lib/p.dart".to_string(), "invalid_export_of_internal_element".to_string(), 1)]);
    assert!(r.records[0].problem_message.contains("'B'"), "{}", r.records[0].problem_message);
}

/// A `@literal` constructor called without `const` in a constant context is
/// already `const` (`InstanceCreationExpression.isConst`; ngrouter's
/// `const routerModule = Module(...)`; checked against `dart analyze` 3.6.2,
/// 2026-10-13).
#[test]
fn literal_constructor_in_a_constant_context() {
    let Ok(engine) = Engine::discover() else {
        eprintln!("SDK missing: test skipped");
        return;
    };
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repo.join(format!("target/tmp-agent/literal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::write(root.join("pubspec.yaml"), "name: literal\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n").unwrap();
    let meta = std::fs::canonicalize(repo.join("corpus/diagnosticos/pacotes/meta")).unwrap();
    let cfg = serde_json::json!({ "configVersion": 2, "packages": [
        { "name": "literal", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.6" },
        { "name": "meta", "rootUri": format!("file://{}", meta.display()), "packageUri": "lib/", "languageVersion": "2.12" },
    ]});
    std::fs::write(root.join(".dart_tool/package_config.json"), cfg.to_string()).unwrap();
    std::fs::write(
        root.join("lib/a.dart"),
        "import 'package:meta/meta.dart';\nclass M {\n  @literal\n  const M();\n}\nconst m = M();\nfinal n = M();\nvar l = const [M()];\nfinal o = [M()];\n",
    )
    .unwrap();
    let sources = vec![root.join("lib/a.dart")];
    let ex = Execution { workers: 1, batch_size: 1, progress: false, isolate: None, max_time: std::time::Duration::from_secs(600) };
    let r = run_ours(&engine, &root, &sources, Some(&root.join(".dart_tool/package_config.json")), &filters::AnalysisOptions::default(), &ex);
    let _ = std::fs::remove_dir_all(&root);
    let found: Vec<(String, usize, usize)> = r.records.iter().map(|x| (x.code.clone(), x.line, x.column)).collect();
    let code = "non_const_call_to_literal_constructor".to_string();
    assert_eq!(found, vec![(code.clone(), 7, 11), (code, 9, 12)]);
}
