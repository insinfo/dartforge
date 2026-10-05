import os,io
base=os.path.dirname(os.path.abspath(__file__))
C={}
# ---------- elementos de topo ----------
C['u/u01_classe.dart']=r"""class _A {}
class _B { _B? outro; }
class _C {}
class _D {}
class _E {}
class _F {}
class _G {}
class _H {}
class _I {}
class K { _I? campo; }
_E? topo;
void f(Object o, _F p) {
  _C? local;
  List<_D> lista = [];
  o is _G;
  o as _H;
  print([local, lista]);
}
"""
C['u/u02_funcoes.dart']=r"""void _naoUsada() {}
void _recursiva() { _recursiva(); }
void _a() { _b(); }
void _b() { _a(); }
void _tearoff() {}
/// Veja [_soDoc].
void _soDoc() {}
void _chamada() {}
void main() {
  var t = _tearoff;
  _chamada();
  print(t);
}
"""
C['u/u03_variaveis_de_topo.dart']=r"""int _nunca = 0;
int _soEscrita = 0;
int _lida = 0;
int _incremento = 0;
int _composta = 0;
int? _seNula;
int _incEmExpr = 0;
int get _g => 0;
set _s(int v) {}
int get _g2 => 0;
set _s2(int v) {}
int get _par => 0;
set _par(int v) {}
void main() {
  _soEscrita = 1;
  print(_lida);
  _incremento++;
  _composta += 2;
  _seNula ??= 3;
  print(_incEmExpr++);
  print(_g);
  _s = 1;
  _par += 1;
}
"""
C['u/u04_metodos.dart']=r"""class A {
  void _naoUsado() {}
  void _usadoPorThis() {}
  void _soDinamico() {}
  int _campoDinamico = 0;
  void _recursivo() { _recursivo(); }
  static void _estatico() {}
  void _tearoff() {}
  void m() {
    this._usadoPorThis();
    print(_tearoff);
  }
}
void f(dynamic d) {
  d._soDinamico();
  print(d._campoDinamico);
}
"""
C['u/u05_sobrescrita.dart']=r"""class A {
  void _m() {}
  void _n() {}
  int get _g => 0;
}
class B extends A {
  @override
  void _m() {}
  @override
  void _n() {}
  @override
  int get _g => 1;
}
class C extends B {
  @override
  void _m() {}
}
void f(A a) {
  a._m();
  print(a._g);
}
"""
C['u/u06_classe_privada.dart']=r"""class _P {
  void instancia() {}
  static void estatico() {}
  static void estaticoUsado() {}
  static int campoEstatico = 0;
  static int campoEstaticoLido = 0;
  int campoInstancia = 0;
  _P();
  _P.nomeado();
  _P.usado();
}
mixin _M {
  static void s() {}
  void i() {}
}
class Q with _M {}
void main() {
  _P.estaticoUsado();
  print(_P.campoEstaticoLido);
  _P.usado();
}
"""
C['u/u07_getter_setter.dart']=r"""class A {
  int get _soGetterLido => 0;
  int get _soGetterNaoLido => 0;
  set _soSetterEscrito(int v) {}
  set _soSetterNaoEscrito(int v) {}
  int get _parEscrito => 0;
  set _parEscrito(int v) {}
  int get _parComposto => 0;
  set _parComposto(int v) {}
  int get _parLido => 0;
  set _parLido(int v) {}
  void m() {
    print(_soGetterLido);
    _soSetterEscrito = 1;
    _parEscrito = 2;
    _parComposto += 3;
    print(_parLido);
  }
}
"""
C['u/u08_construtores.dart']=r"""class A {
  A();
  A._naoUsado();
  A._usado();
  A._porRedirecionamento();
  A._porSuper();
  A._porFabrica();
  A.r() : this._porRedirecionamento();
  factory A.f() = A._porFabrica;
}
class B extends A {
  B() : super._porSuper();
}
class Unico {
  Unico._();
}
class _Priv {
  _Priv.new();
  _Priv.n();
  _Priv.publicoNaoUsado();
}
class Tear {
  Tear();
  Tear._t();
}
void main() {
  A._usado();
  _Priv();
  _Priv.n();
  print(Tear._t);
}
"""
C['u/u09_enum.dart']=r"""enum _NaoUsado { a }
enum _E { usado, naoUsado }
enum _V { x, y }
enum Pub { a, _priv, b }
enum Ctor {
  a, b.n();
  const Ctor();
  const Ctor.n();
  const Ctor.naoUsado();
  factory Ctor.f() => a;
}
void main() {
  print(_E.usado);
  print(_V.values);
}
"""
C['u/u10_extensao.dart']=r"""extension _Priv on int {
  int get naoUsado => 0;
  int get usado => 1;
  void metodo() {}
  int operator +(int o) => 0;
  static void est() {}
}
extension on String {
  void semNomeNaoUsado() {}
  void semNomeUsado() {}
  void _privado() {}
}
extension Pub on double {
  void publico() {}
  void _privNaoUsado() {}
  void _privUsado() {}
  static void _est() {}
}
void main() {
  print(1.usado);
  ''.semNomeUsado();
  1.5._privUsado();
}
"""
C['u/u11_tipo_de_extensao.dart']=r"""extension type _NaoUsado(int v) {}
extension type _Usado(int v) {}
extension type _SoTipoLocal(int v) {}
extension type Pub._(int _v) {
  Pub.outro(this._v);
  Pub._priv(this._v);
  void _m() {}
}
extension type _Dois(int v) {
  _Dois.n(this.v);
}
void main() {
  print(_Usado(1));
  _SoTipoLocal? x;
  print(x);
  print(_Dois.n(1));
}
"""
C['u/u12_typedef.dart']=r"""typedef _NaoUsado = int;
typedef _Usado = int;
typedef void _Antigo();
typedef _SoLocal = String;
class _C {
  _C();
  _C.nomeado();
  _C._priv();
}
typedef Exposto = _C;
class _D {
  _D();
  _D.nomeado();
}
typedef _NaoExposto = _D;
_Usado f() {
  _SoLocal? s;
  print(s);
  return 0;
}
"""
C['u/u13_mixin.dart']=r"""mixin _NaoUsado {}
mixin _Usado {}
mixin _SoOn on Object {}
class A with _Usado {}
mixin B on _SoOn {}
"""
C['u/u14_pragma.dart']=r"""@pragma('vm:entry-point')
void _entrada() {}
@pragma('vm:prefer-inline')
void _outra() {}
@pragma('vm:entry-point')
class _C {
  @pragma('vm:entry-point')
  void _m() {}
  @pragma('vm:entry-point')
  int _campo = 0;
  int _campo2 = 0;
  void _m2() {}
}
"""
C['u/u15_funcoes_locais.dart']=r"""void main() {
  void naoUsada() {}
  void recursiva() { recursiva(); }
  void usada() {}
  void _() {}
  void __() {}
  void a() { void b() {} }
  var c = () {};
  usada();
  a();
}
"""
C['u/u16_parametros.dart']=r"""void _f1([int? a]) {}
void _f2([int? a]) {}
void _f3({int? a, int? b}) {}
void _f4<T>([int? a]) {}
void _f5([int? a]) {}
void pub([int? a]) {}
class _C {
  _C([int? a]);
  _C.n({int? a});
  void m([int? a]) {}
  static void s([int? a]) {}
}
class G<T> {
  G._([int? a]);
}
class S {
  S._(int a);
  S._o([int? a]);
}
class T extends S {
  T._([super.a]) : super._();
  T._o([super.a]) : super._o();
  T._p([int? a]) : super._(a ?? 0);
}
class Base {
  void _m([int? a]) {}
}
class Der extends Base {
  @override
  void _m([int? a, int? b]) {}
}
void main() {
  _f1();
  _f2(1);
  _f3(a: 1);
  _f4();
  print(_f5);
  _C();
  _C.n();
  _C.s();
  G._();
  T._();
  T._o();
  T._p();
  Base()._m(1);
  Der()._m();
}
"""
C['u/u17_campos.dart']=r"""class A {
  int _nunca = 0;
  int _soEscrito = 0;
  int _lido = 0;
  int _inc = 0;
  int _incThis = 0;
  int _composto = 0;
  int? _seNulo;
  int _incEmExpr = 0;
  final int _soInicializador;
  final int _soFormal;
  int _padrao = 0;
  int _relacional = 0;
  static int _estatico = 0;
  int _viaOutro = 0;
  A(this._soFormal) : _soInicializador = 1;
  void m(A o) {
    _soEscrito = 1;
    print(_lido);
    _inc++;
    this._incThis++;
    _composto += 1;
    _seNulo ??= 1;
    print(_incEmExpr++);
    o._viaOutro = 2;
    if (o case A(_padrao: 1)) {}
  }
}
"""
C['u/u18_locais.dart']=r"""void f(Object o, List<int> l) {
  var nunca = 0;
  var soEscrita = 0;
  var lida = 0;
  var inc = 0;
  var neg = 0;
  var naoNula = 0 as int?;
  var composta = 0;
  int? seNula;
  var emClosure = 0;
  var _ = 0;
  var __ = 0;
  late int tardia;
  soEscrita = 1;
  print(lida);
  inc++;
  -neg;
  naoNula!;
  composta += 1;
  seNula ??= 1;
  () { emClosure = 2; };
  tardia = 3;
  for (var i = 0; ;) {}
  for (var e in l) {}
  for (var j = 0, k = 0; j < 1; j++) {}
}
"""
C['u/u19_padroes.dart']=r"""void f(Object o, (int, int) r) {
  var (a, b) = r;
  var (c, d) = r;
  var [e, _] = [1, 2];
  final (g, h) = r;
  print(c);
  if (o case int x) {}
  if (o case [int y, int z]) { print(y); }
  switch (o) {
    case int p || [int p]:
      break;
    case (int q, int w) || [int q, int w]:
      print(q);
  }
  switch (o) {
    case int m when m > 0:
    case [int m]:
      break;
  }
  var s = switch (o) { int n => 1, String t => t.length, _ => 0 };
  print(s);
  int u, v;
  (u, v) = r;
}
"""
C['u/u20_catch.dart']=r"""void f() {
  try {} catch (e) {}
  try {} catch (e, s) {}
  try {} on Exception catch (e) {}
  try {} on Exception catch (e, s) {}
  try {} on Exception catch (e, s) { print(e); }
  try {} on Exception catch (e, s) { print(s); }
  try {} on Exception catch (_) {}
  try {} catch (_, __) {}
}
"""
C['u/u21_erro_de_sintaxe.dart']=r"""import 'dart:math';
class _NaoUsada {}
void _f() {}
class A { int _campo = 0; }
void main() {
  var x = 0;
  var y = ;
  int z = 1
}
"""
C['u/u22_operadores.dart']=r"""class A {
  int _v = 0;
}
extension _E on A {
  A operator +(int o) => this;
  A operator -(int o) => this;
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator -() => this;
  A operator ~() => this;
  int call() => 0;
  bool operator <(int o) => true;
}
extension _F on A {
  int operator [](int i) => 0;
  void operator []=(int i, int v) {}
  A operator *(int o) => this;
}
void f(A a, _F? nada) {
  a + 1;
  a[0];
  -a;
  a();
  var b = a;
  b *= 2;
  _F(a)[0] += 1;
  print(a._v);
}
"""
C['u/u23_this_super.dart']=r"""class A {
  int _a = 0;
  int _b = 0;
  void _m() {}
  void _n() {}
}
class B extends A {
  void x() {
    print(super._a);
    super._m();
  }
}
/// [A._b] e [A._n] só em comentário.
void f() {}
"""
C['u/u24_anotacao.dart']=r"""class _Anot { const _Anot(); }
const _c = 1;
const _d = 2;
class _SoAnot { const _SoAnot.n(); const _SoAnot.o(); }
@_Anot()
@_SoAnot.n()
void f([@_c int? x]) {}
"""
C['u/u25_nomes_iguais.dart']=r"""class A {
  void _m() {}
  int _f = 0;
}
class B {
  void _m() {}
  int _f = 0;
}
void f(B b) {
  b._m();
  print(b._f);
}
"""
C['u/u26_generico.dart']=r"""class A<T> {
  T? _f;
  void _m() {}
  int operator +(int o) => 0;
}
extension _E<T> on List<T> {
  void _x() {}
  List<T> operator -(int o) => this;
}
void f(A<int> a, List<int> l) {
  print(a._f);
  a._m();
  l._x();
  var m = l;
  m -= 1;
}
"""
# ---------- parts ----------
C['parte/lib.dart']=r"""import 'dart:math';
import 'dart:collection';
part 'p.dart';
class _SoNaParte {}
class _Nunca {}
void _f() {}
int _campoTopo = 0;
"""
C['parte/p.dart']=r"""part of 'lib.dart';
void g() {
  _SoNaParte();
  _f();
  print(pi);
  var naoUsada = _campoTopo;
}
class _NaParte {}
"""
# ---------- rótulos ----------
C['r/r01_rotulos.dart']=r"""void f(int x) {
  naoUsado: while (true) { break; }
  usadoBreak: while (true) { break usadoBreak; }
  usadoContinue: for (;;) { continue usadoContinue; }
  bloco: { print(1); }
  a: b: while (true) { break b; }
  fora: while (true) {
    dentro: while (true) { break fora; }
  }
  switch (x) {
    c0: case 0:
      continue c1;
    c1: case 1:
      break;
    c2: default:
      break;
  }
  repetido: while (true) {
    repetido: while (true) { break repetido; }
  }
  closure: while (true) {
    () { break closure; };
    break;
  }
  switch (x) { case 1: interno: print(2); break; }
}
"""
# ---------- depreciados ----------
C['d/d_lib.dart']=r"""@deprecated
void velha() {}
@Deprecated('Use nova')
void comMensagem() {}
@Deprecated('Use nova.')
void comPonto() {}
@Deprecated('  ')
void comVazia() {}
class C {
  @deprecated
  C();
  @deprecated
  C.n();
  @deprecated
  int campo = 0;
  @deprecated
  int get g => 0;
  @deprecated
  set s(int v) {}
  @deprecated
  int operator +(int o) => 0;
  @deprecated
  int call() => 0;
  void m({@deprecated int? p, @Deprecated('nao') int? q}) {}
}
@deprecated
class Velha {}
@deprecated
const velhaConst = 1;
"""
C['d/d01_usos.dart']=r"""import 'd_lib.dart';
import 'd_lib.dart' as p show velha;
import 'd_lib.dart' as h hide velha;
void f(C c, Velha v) {
  velha();
  comMensagem();
  comPonto();
  comVazia();
  C();
  C.n();
  c.campo;
  c.campo = 1;
  c.campo += 1;
  c.g;
  c.s = 1;
  c + 1;
  c();
  c.m(p: 1, q: 2);
  print(velhaConst);
  print(velha);
  p.velha();
  print(C.new);
}
@deprecated
void g() { velha(); }
@deprecated
class D { void m() { velha(); } }
/// [velha] em comentário.
void h() {}
"""
C['d/d02_mesma_biblioteca.dart']=r"""@deprecated
void _velha() {}
@deprecated
int x = 0;
void f({@deprecated int? p}) { print(p); }
void main() {
  _velha();
  x = 1;
  x++;
  f(p: 1);
}
"""
for n,s in C.items():
    p=os.path.join(base,n); os.makedirs(os.path.dirname(p),exist_ok=True)
    io.open(p,'w',encoding='utf-8',newline='\n').write(s)
print(len(C),'arquivos')
