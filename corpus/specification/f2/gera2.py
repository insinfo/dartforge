import os,io,shutil
base=os.path.dirname(os.path.abspath(__file__))
C={}
# ---------- bibliotecas de apoio ----------
C['imp/lib_a.dart']=r"""class A {}
class A2 {}
int topo = 0;
void fa() {}
extension ExtA on int { int get dobro => this * 2; }
"""
C['imp/lib_b.dart']=r"""export 'lib_a.dart';
class B {}
"""
C['imp/lib_c.dart']=r"""class C {}
extension ExtC on String { int get tam => length; }
"""
C['imp/lib_e.dart']=r"""extension SoExt on int { int get triplo => this * 3; }
"""
C['imp/lib_s.dart']=r"""set soSetter(int v) {}
const konst = 1;
"""
C['imp/lib_f.dart']=r"""import 'lib_a.dart';
A criaA() => A();
"""
C['imp/lib_x.dart']=r"""class A {}
class Future {}
"""
# ---------- casos ----------
C['imp/i01.dart']=r"""import 'lib_a.dart';
import 'lib_c.dart';
void f(A a) {}
"""
C['imp/i02.dart']=r"""import 'lib_a.dart';
import 'lib_b.dart';
void f(A a) {}
"""
C['imp/i03.dart']=r"""import 'lib_a.dart';
import 'lib_b.dart';
void f(A a, B b) {}
"""
C['imp/i04.dart']=r"""import 'lib_a.dart';
import 'lib_a.dart';
void f(A a) {}
"""
C['imp/i05.dart']=r"""import 'lib_a.dart';
import 'lib_a.dart';
"""
C['imp/i06.dart']=r"""import 'lib_a.dart' show A, A2, topo, fa;
void f(A a) { fa(); }
"""
C['imp/i07.dart']=r"""import 'lib_a.dart' show A, A2;
"""
C['imp/i08.dart']=r"""import 'lib_a.dart' as p;
import 'lib_c.dart' as p;
import 'lib_b.dart' as q;
void f(p.A a) {}
"""
C['imp/i09.dart']=r"""import 'lib_e.dart';
import 'lib_c.dart';
void f() { print(1.triplo); }
"""
C['imp/i10.dart']=r"""import 'lib_e.dart';
void f() {}
"""
C['imp/i11.dart']=r"""import 'lib_e.dart' as p;
void f() { print(1.triplo); }
"""
C['imp/i12.dart']=r"""import 'lib_e.dart';
import 'lib_a.dart';
void f() { print(SoExt(1).triplo); print(ExtA(2).dobro); }
"""
C['imp/i13.dart']=r"""import 'lib_a.dart' deferred as d;
void f() { d.loadLibrary(); }
"""
C['imp/i14.dart']=r"""import 'dart:core';
import 'dart:core' as core;
void f() {}
"""
C['imp/i15.dart']=r"""import 'nao_existe.dart';
import 'lib_c.dart';
void f() {}
"""
C['imp/i16.dart']=r"""import 'lib_a.dart';
import 'lib_c.dart' as p;
import 'lib_b.dart' as q;
import 'lib_e.dart' as r;
/// [A] e [p].
/// [q.B]
void f() {}
"""
C['imp/i17.dart']=r"""import 'lib_c.dart';
void f() { naoExiste; }
"""
C['imp/i18.dart']=r"""import 'lib_c.dart' as _;
import 'lib_a.dart' as _;
void f(_.C c) {}
"""
C['imp/i19.dart']=r"""import 'lib_a.dart';
class A {}
void f(A a) {}
"""
C['imp/i20.dart']=r"""import 'lib_a.dart';
export 'lib_a.dart';
export 'lib_a.dart';
"""
C['imp/i21.dart']=r"""import 'dart:async';
Future<void> f() async {}
"""
C['imp/i21b.dart']=r"""import 'dart:async';
int f() => 0;
"""
C['imp/i21c.dart']=r"""import 'dart:async';
import 'dart:collection';
void f(Completer<int> c, List<int> l) { print(l.firstOrNull); }
"""
C['imp/i22.dart']=r"""import 'lib_a.dart';
import 'lib_a.dart' as p;
void f(A a, p.A b) {}
"""
C['imp/i23.dart']=r"""import 'lib_s.dart';
void f() { soSetter = 1; }
"""
C['imp/i24.dart']=r"""import 'lib_a.dart' show ExtA, A;
void f() { print(1.dobro); }
"""
C['imp/i25.dart']=r"""import 'lib_a.dart' hide A;
import 'lib_c.dart' hide C;
void f() { fa(); }
"""
C['imp/i26.dart']=r"""import 'lib_a.dart';
void f(int A) { print(A); }
"""
C['imp/i27.dart']=r"""import 'lib_c.dart' as p;
void f(int p) { p.toString(); }
"""
C['imp/i29.dart']=r"""import 'lib_s.dart';
@konst
void f() {}
"""
C['imp/i30.dart']=r"""import 'lib_c.dart' as p;
import 'lib_a.dart';
void f() { p; }
"""
C['imp/i31.dart']=r"""import 'lib_a.dart' show A;
import 'lib_a.dart';
void f(A a) { fa(); }
"""
C['imp/i32.dart']=r"""import 'lib_a.dart';
import 'lib_f.dart';
void f() { var x = criaA(); print(x); }
"""
C['imp/i37.dart']=r"""import 'lib_a.dart' if (dart.library.io) 'lib_c.dart';
void f() {}
"""
C['imp/i38.dart']=r"""import "dart:math";
import r'dart:collection';
import 'dart:' 'typed_data';
import '''dart:convert''';
void f() {}
"""
C['imp/i39.dart']=r"""import 'lib_a.dart' show A;
import 'lib_a.dart' show A2;
import './lib_a.dart' show A;
void f(A a, A2 b) {}
"""
C['imp/i40.dart']=r"""import 'lib_a.dart';
import 'lib_x.dart';
import 'lib_c.dart';
void f(A a) {}
"""
C['imp/i41.dart']=r"""import 'dart:async';
import 'lib_x.dart';
Future? x;
"""
C['imp/i42.dart']=r"""import 'lib_a.dart';
import 'lib_c.dart';
void f() {
  var x = 0
}
"""
C['imp/i43.dart']=r"""import 'lib_a.dart' as p;
import 'lib_b.dart' as p;
void f(p.A a, p.B b) {}
"""
C['imp/i44.dart']=r"""import 'lib_a.dart';
import 'lib_c.dart';
void f() { print(''.tam); print(''.naoExiste); }
"""
C['imp/i45.dart']=r"""import 'lib_a.dart';
import 'lib_e.dart';
void f() { print(1.dobro); }
"""
# ---------- elementos: correções e complementos ----------
C['u2/u22b_operadores.dart']=r"""class A {}
extension _E on A {
  A operator +(int o) => this;
  A operator -(int o) => this;
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator -() => this;
  A operator ~() => this;
  int call() => 0;
  bool operator <(int o) => true;
  A operator *(int o) => this;
  A operator /(int o) => this;
}
class B {}
extension _F on B {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
class D {}
extension _G on D {
  int? operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
class H {}
extension _I on H {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
}
void f(A a, B b, D d, H h) {
  a + 1;
  a[0];
  -a;
  a();
  var c = a;
  c *= 2;
  print(c);
  b[0] += 1;
  d[0] ??= 1;
  h[0] = 1;
}
"""
C['u2/u27_alias_de_classe.dart']=r"""class S {}
mixin M {}
class _Alias = S with M;
class _Base {}
class _Derivada extends _Base {}
void _a() {}
void _b() { _a(); }
"""
C['u2/u28_campo_sobrescreve.dart']=r"""class A {
  int get _x => 0;
  int _y = 0;
}
class B extends A {
  @override
  int _x = 1;
  @override
  int _y = 2;
  int _z = 3;
}
void f(A a, dynamic d) {
  print(a._x);
  print(d._z);
}
"""
C['u2/u29_for_in_identificador.dart']=r"""void f(List<int> l) {
  int x;
  for (x in l) {}
  int y = 0;
  for (y in l) { print(y); }
  var s = 'a';
  var t = 'b';
  print('$s');
  assert(t == 'b');
}
"""
C['u2/u30_enum_values.dart']=r"""enum Pub { a, _priv }
enum _E { x, y }
enum _F { x, y }
void main() {
  print(Pub.values);
  print(_E.x.index);
  for (var e in _F.values) { print(e); }
}
"""
C['u2/u31_construtor_padrao.dart']=r"""class _A {
  _A();
  _A.n();
}
class _B extends _A {
  _B() : super();
  _B.n() : super.n();
}
class _C {
  _C.new();
  _C.n();
}
void main() {
  _B();
  _B.n();
  _C.n();
}
"""
C['u2/u32_rotulos.dart']=r"""void f(int x, bool c) {
  naoUsado: while (c) { break; }
  usadoBreak: while (c) { break usadoBreak; }
  usadoContinue: for (; c;) { continue usadoContinue; }
  bloco: { print(1); }
  a: b: while (c) { break b; }
  fora: while (c) {
    dentro: while (c) { break fora; }
  }
  switch (x) {
    c0: case 0:
      continue c1;
    c1: case 1:
      break;
    c2: default:
      break;
  }
  repetido: while (c) {
    repetido: while (c) { break repetido; }
  }
  switch (x) { case 1: interno: print(2); break; }
  semAlvo: while (c) { break naoExiste; }
}
"""
C['u2/u33_corpo_sem_bloco.dart']=r"""void f(bool c, Object o) {
  while (c) var x = 0;
  if (c) var y = 1; else var z = 2;
  for (var (p) = 0; c;) {}
  for (var (q, r) = (0, 1); q < 1;) {}
  int w;
  [w, final v] = [1, 2];
}
"""
C['u2/u34_mixin_construtor.dart']=r"""abstract class Foo {
  factory Foo({required String thing}) = _Foo._;
  Foo._({required this.thing});
  final String thing;
  void bar();
}
mixin _$Foo on Foo {
  @override
  void bar() {}
}
class _Foo = Foo with _$Foo;
"""
C['u2/u35_campo_sombreado.dart']=r"""class A {
  int _ = 0;
  int _x = 0;
  void m(int _) { print(_); }
  void n() { var _x = 1; print(_x); }
  A(int _x, {int? y});
  A.o() { A(1, y: 2); }
}
"""
C['u2/u36_topo_composto.dart']=r"""int _a = 1;
int _b = 1;
int _c = 1;
f() {
  _a += 1;
  _b++;
  print(_c += 1);
}
"""
C['u2/u37_parametro_tipo.dart']=r"""// @dart=3.5
class A<_> {
  _() {}
}
class B<_T> {
  void _m<_U>() {}
}
"""
C['d2/d_lib.dart']=C_d=r"""@deprecated
void velha() {}
@Deprecated('Use nova')
void comMensagem() {}
"""
C['d2/d03.dart']=r"""import 'd_lib.dart' show velha;
import 'd_lib.dart' as q hide velha;
/// [velha] em comentário.
void f() { q.comMensagem(); }
"""
C['d2/sdk.dart']=r"""import 'dart:io';
import 'dart:collection';
void f(List<int> l) {
  print(Platform.packageRoot);
  print(DateTime.utc(1).isAfter);
  print(HasNextIterator);
}
"""
# ---------- 3.13.4 ----------
C['n313/p01_primario.dart']=r"""class A(final int _naoLido, var int _lido, final int publico, int _soParametro) {
  int get g => _lido;
}
class B(final int _x) {
  void m() { _x; }
}
class C(var int _y) {
  void m() { _y = 1; _y++; }
}
enum E(final int _v, final int _w) {
  a(1, 2);
  int get w => _w;
}
extension type T(int _r) {}
class D(final int _, final int __) {}
"""
C['n313/p02_parametro.dart']=r"""void _f([int? a]) {}
class _C([int? a, this.b]) {
  int? b;
  _C.n({int? c});
}
enum E([int? x]) { a }
void main() {
  _f();
  _C();
  _C.n();
}
"""
C['n313/p03_local_curinga.dart']=r"""void main() {
  void _() {}
  void __() {}
  var _ = 0;
  var __ = 0;
}
"""
for n,s in C.items():
    p=os.path.join(base,n); os.makedirs(os.path.dirname(p),exist_ok=True)
    io.open(p,'w',encoding='utf-8',newline='\n').write(s)
# cópias do corpus
corp='E:/MyRustProjects/dartforge/corpus/diagnosticos/'
cop=['analyzer/unused_element/UnusedElement__extension_unnamed_operat_83235816.dart',
'analyzer/unused_element/UnusedElement__method_isUsed_privateExt_75655fd4.dart',
'analyzer/unused_element/UnusedElement__method_isUsed_privateExt_d8f24a17.dart',
'analyzer/unused_element/UnusedElement__getter_notUsed_invocatio_7d9034d5.dart',
'analyzer/unused_element/UnusedElement__parameter_isUsed_overrid_387a0899.dart',
'analyzer/unused_element/UnusedElement__classPrivate_secondaryCo_2f78d312.dart',
'analyzer/unused_local_variable/UnusedLocalVariable__switchStatement_sh_8d5b3b84.dart',
'analyzer/unused_local_variable/UnusedLocalVariable__switchStatement_sh_b7ecf69a.dart',
'analyzer/unused_field/UnusedField__isUsed_underscoreField_shadowsLocal.dart',
'analyzer/label_undefined/LabelUndefined__break.dart',
'analyzer/use_of_private_parameter_name/UseOfPrivateParameterName__andVoidLhsError.dart',
'analyzer/duplicate_definition/DuplicateDefinition__parameters_constru_072562a6.dart',
]
os.makedirs(os.path.join(base,'corpus'),exist_ok=True)
for c in cop:
    shutil.copy(corp+c, os.path.join(base,'corpus',os.path.basename(c)))
print(len(C),'arquivos',len(cop),'cópias')
