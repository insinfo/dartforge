import os
B = r'E:\dftemp\analise\spec-r4\casos\f1'
C = {}
C['s01.dart'] = '''void f() {
  x;
}
'''
C['s02.dart'] = '''set foo(int _) {}
void f() {
  foo;
  foo += 1;
  foo++;
  foo = 2;
}
'''
C['s03.dart'] = '''class A {
  set foo(int _) {}
  static set bar(int _) {}
  void m() {
    foo;
    bar;
    foo();
    bar();
  }
}
'''
C['s04.dart'] = '''void f() {
  x = 1;
  y += 1;
  z++;
  for (w in []) {}
}
'''
C['s05.dart'] = '''void f() {
  g();
  g2<int>(1);
}
class A {
  void m() {
    g();
  }
  static void s() {
    g();
  }
}
extension E on int {
  void m() {
    g();
  }
}
var v = g();
'''
C['s06.dart'] = '''class A {
  int x = 0;
  void im() {}
  static void s() {
    x;
    im();
    x = 1;
  }
  factory A.f() {
    x;
    return throw 0;
  }
  A() : y = x;
  int y;
}
'''
C['s07.dart'] = '''class A {
  static int s = 0;
  static void sm() {}
}
class B extends A {
  void m() {
    s;
    sm();
    s = 1;
    sm;
  }
}
extension E on A {
  void m() {
    s;
    sm();
  }
}
'''
C['s08.dart'] = '''void a() { await; }
var t = await;
class C {
  var f = await;
  void m() { await; }
}
void b() { await x; }
'''
C['s09.dart'] = '''void f() {
  await v;
  await<int> w;
}
await g() => throw 0;
'''
C['s10.dart'] = '''import 'nao_existe.dart' as p;
import 'nao_existe2.dart' show A, g;
p.T a = throw 0;
A b = throw 0;
B c = throw 0;
void f() {
  p.x;
  p.h();
  p.x = 1;
  new p.K();
  A;
  g();
  x;
  h();
  A();
  B();
}
class D extends p.S implements A {}
'''
C['s11.dart'] = '''part 's11.g.dart';
_$A a = throw 0;
_B b = throw 0;
void f() {
  _$x;
  _$g();
  new _$A();
}
'''
C['s12.dart'] = '''class C {
  factory C.r() = Undef;
  C();
}
Undef v1 = throw 0;
void f(Object o) {
  try {} on Undef catch (e) {}
  o as Undef;
  o is Undef;
  o is! Undef;
  <Undef>[];
  List<Undef> l;
  new Undef();
  const Undef();
  Undef();
  Undef<int>();
  Undef.named();
  Undef<int>.named();
}
class D1 extends Undef {}
class D2 implements Undef {}
class D3 with Undef {}
mixin M on Undef {}
class D4 = Object with Undef;
class D5 = Undef with D1;
extension E on Undef {}
typedef T = Undef;
typedef F = void Function(Undef);
void g<X extends Undef>(Undef p, {Undef? q}) {}
'''
C['s13.dart'] = '''int v = 0;
void fn() {}
int get gt => 0;
class C {
  factory C.r() = v;
  C();
}
v v1 = throw 0;
fn v2 = throw 0;
gt v3 = throw 0;
void f(Object o) {
  try {} on v catch (e) {}
  o as v;
  o is v;
  <v>[];
  new v();
  const v();
  new fn();
}
class D1 extends v {}
class D2 implements fn {}
class D3 with v {}
mixin M on v {}
'''
C['s14.dart'] = '''void f(int par, Object o) {
  int loc = 0;
  void lf() {}
  loc a;
  lf b;
  par c;
  o is loc;
  o is par;
  <loc>[];
  new loc();
  later d;
  int later = 0;
}
'''
C['s15.dart'] = '''boolean b = true;
void f(Object o) {
  o is boolean;
  <boolean>[];
  new boolean();
}
'''
C['s16.dart'] = '''import 'dart:async' as p;
void top() {}
int topv = 0;
class A<T> {
  int get g => 0;
  set s(int _) {}
  void m() {}
  T.X f1 = throw 0;
  g.X f2 = throw 0;
  s.X f3 = throw 0;
  m.X f4 = throw 0;
}
top.X v1 = throw 0;
topv.X v2 = throw 0;
dynamic.X v3 = throw 0;
void f(int p) {
  p.Future? a;
  new p.Future.value(0);
  const p.X();
  new top.X();
  new top.X.n();
  <p.Future>[];
  Object() is p.Future;
}
p.Future? r(int p) => null;
void g(p.Future? x, int p) {}
void h() {
  p.Future? a;
  var p = 0;
}
'''
C['s17.dart'] = '''class A {
  A.foo();
  static void bar() {}
}
typedef TA = A;
A.foo v1 = throw 0;
A.bar v2 = throw 0;
A.zzz v3 = throw 0;
TA.foo v4 = throw 0;
void f(Object o) {
  o is A.foo;
  o as A.foo;
  <A.foo>[];
  try {} on A.foo catch (e) {}
  new A.foo.x();
  const A.foo.x();
  new A.foo();
  A<int>.foo();
  new A.foo<int>();
}
class B extends A.foo {}
'''
C['s18.dart'] = '''import 'dart:math' as p;
void g(Object? o) {}
void f() {
  p;
  p();
  p = 1;
  p += 1;
  p++;
  p ??= 1;
  p?.pi;
  p?.max(1, 2);
  p[0];
  g(p);
  p..toString();
  p.pi;
  p.max(1, 2);
  p == p;
  for (p in []) {}
}
'''
C['s19.dart'] = '''import 'dart:math' as p;
void f() {
  p.x;
  p.x = 1;
  p.x += 1;
  p.x();
  p.pi = 1;
  p.max = 1;
  p.Random = 1;
  p._x;
  p.x.y;
  p.x.y();
  p.X.y();
  p.X<int>.y();
}
@p.x
void g() {}
@p.x()
void h() {}
'''
C['s21.dart'] = '''import 'dart:html';
import 'dart:io';
void f() {
  File;
  File('a');
}
File? g;
'''
C['s24.dart'] = '''import 'dart:math' deferred as m;
import 'dart:async' deferred as a;
import 'dart:collection' as a;
m.Random? v1;
List<m.Random>? v2;
m.Random? f1(m.Random p, [m.Random? q]) => null;
typedef T = m.Random;
typedef F = m.Random Function(m.Random);
class C<X extends m.Random> {
  m.Random? fld;
  C(m.Random this.fld);
  m.Random get g => throw 0;
  factory C.r() = D<m.Random>;
}
class D<X> implements C<Never> {
  m.Random? fld;
  m.Random get g => throw 0;
}
void f(Object o) {
  o is m.Random;
  o as m.Random;
  try {} on m.Random catch (e) {}
  m.Random? loc;
  <m.Random>[];
  new m.Random();
  for (m.Random x in []) {}
  (m.Random, int)? rec;
  if (o case m.Random x) {}
  m.Undef? u;
  a.Future? af;
}
'''
C['s25.dart'] = '''import 'dart:math' show FooBar, pi hide Baz;
import 'dart:async' hide Qux;
export 'dart:math' show Zed, max;
import 'nao_existe.dart' show Nope;
var x = pi;
'''
C['s26.dart'] = '''import 'dart:core' as core;
dynamic a;
int b = 0;
Never c() => throw 0;
core.dynamic d;
core.Never e() => throw 0;
core.int f = 0;
void g() {
  dynamic;
  print(0);
  core.print(0);
}
'''
C['s27.dart'] = '''import 'dart:core' show List, int;
main() {
  List;
  String;
  dynamic;
  print(1);
}
String? s;
dynamic d;
int i = 0;
'''
C['s28.dart'] = '''int topv = 0;
void topf<T>() {}
typedef TF<T> = void Function(T);
typedef TD<T> = T;
class A<T> {
  A.named();
}
f(x) {
  UnresolvedClass<int>.named();
  unresolved.Class<int>.named();
  x.a<int>.b();
  topv<int>.b();
  topf<int>.b();
  TF<int>.b();
  TD<int>.named();
  A<int>.named();
  A<int>.zzz();
}
'''
C['s29.dart'] = '''int x = 0;
void f() {
  x;
  var x = 1;
  {
    y;
  }
  var y = 2;
  z();
  void z() {}
}
void g() {
  w;
  {
    var w = 0;
  }
  w;
}
'''
C['s30.dart'] = '''class A {
  int get foo => 0;
  static int get sfoo => 0;
}
class B extends A {
  set foo(int _) {}
  set sfoo(int _) {}
  void m() {
    foo;
    sfoo;
  }
}
int get top => 0;
class C {
  set top(int _) {}
  static set stop(int _) {}
  void m() {
    top;
    stop;
    top = 1;
  }
  static void s() {
    top;
  }
}
'''
C['s31.dart'] = '''import 'dart:math' as p;
import 'dart:async' as q;
int p = 0;
class q {}
p.Random? a;
q.Future? b;
void f() {
  p.pi;
  p;
  q;
  q.Future;
}
'''
C['s33.dart'] = '''void f() {
  break L;
  L: while (true) {
    () { break L; };
    continue M;
  }
  x: 0;
}
'''
C['s34.dart'] = '''int top = 0;
class K { int fld = 0; void m() { (fld) = 1; } }
void f(Object o, int par) {
  (x) = 0;
  [y, par] = [1, 2];
  (top) = 0;
  switch (o) {
    case foo:
      break;
    case == unresolved:
      break;
    case const (bar):
      break;
  }
  if (o case zed) {}
  if (o case == b && var b) {}
}
'''
C['s35.dart'] = '''class G<T> {}
class H<T> {
  T<Unresolved>? a;
  T<int>? b;
}
G<Unresolved>? v1;
G<Unresolved, int>? v2;
Unresolved<Other>? v3;
G<G<Unresolved>>? v4;
dynamic<Unresolved>? v5;
int topv = 0;
topv<int>? v6;
void f<X>() {
  f<Unresolved>();
  X<Unresolved> l;
  int loc = 0;
  G<loc>? m;
  G<f>? n;
  G<topv>? o;
}
class S1 extends G<Unresolved> {}
class S2 implements G<topv> {}
'''
C['s36.dart'] = '''class Annotation {
  const Annotation(dynamic d);
}
class C<@Annotation(foo) T> {
  static void foo() {}
}
void f<@Annotation(bar) T>(@Annotation(baz) int p) {}
@Annotation(qux)
void g() {}
@undef
void h() {}
@undef.x
void i() {}
class Foo {
  @Annotation(Bar)
  set Bar(int value) {}
}
class Bar {}
'''
C['s39.dart'] = '''import 'dart:math' deferred as m;
import 'dart:async' as a;
import 'nao_existe.dart' deferred as n;
void f() {
  m.loadLibrary;
  m.loadLibrary();
  m.loadLibrary(1);
  a.loadLibrary;
  a.loadLibrary();
  n.loadLibrary();
  n.loadLibrary;
  loadLibrary();
  loadLibrary;
}
'''
C['s41.dart'] = '''class A<T> {
  void m() {
    T;
    T();
    T.x;
    T.x();
    new T();
  }
  static void s() {
    T;
    T? v;
  }
}
'''
C['s42.dart'] = '''void f() {
  x.y;
  x.y();
  x.y = 1;
  x?.y;
  x.y.z;
  x[0];
  x<int>;
  x + 1;
  -x;
  x..a..b();
  x!;
  [x, ...x];
  '$x ${x.y}';
}
'''
C['s43.dart'] = '''class A {
  var a = undef1;
  static var b = undef2;
  final c;
  A() : c = undef3, super();
  A.n(int p) : c = p, assert(undef4);
  void m([x = undef5]) {}
}
var t = undef6;
void f([x = undef7]) {}
enum E { v(undef8); const E(Object o); }
'''
C['s44.dart'] = '''typedef T = int;
extension Ext on int {}
mixin Mx {}
enum En { a }
void fn() {}
void f(Object o) {
  new T();
  new Ext();
  new Mx();
  new En();
  new dynamic();
  new Never();
  new void();
  const fn();
  Ext x1;
  o is Ext;
  <Ext>[];
}
'''
# multi-arquivo: ambiguous import
C['amb/a.dart'] = '''class A {}
int foo = 0;
void fn() {}
set only(int _) {}
int get gs => 0;
'''
C['amb/b.dart'] = '''class A {}
int foo = 0;
void fn() {}
int get only => 0;
set gs(int _) {}
'''
C['amb/c.dart'] = '''export 'a.dart';
'''
C['amb/d.dart'] = '''class A {}
'''
C['amb/main.dart'] = '''import 'a.dart';
import 'b.dart';
A? v1;
List<A>? v2;
class X extends A {}
void f(Object o) {
  A;
  A();
  new A();
  foo;
  foo = 1;
  foo += 1;
  fn();
  fn;
  o is A;
  only;
  only = 1;
  gs;
  gs = 1;
}
'''
C['amb/main2.dart'] = '''import 'a.dart' as p;
import 'b.dart' as p;
p.A? v1;
void f() {
  p.A;
  p.A();
  p.foo;
  p.foo = 1;
  p.fn();
}
'''
C['amb/main3.dart'] = '''import 'a.dart';
import 'c.dart';
A? v1;
void f() {
  foo;
}
'''
C['amb/main4.dart'] = '''import 'c.dart';
import 'b.dart';
import 'd.dart';
A? v1;
'''
C['amb/main5.dart'] = '''import 'sdk_like.dart';
import 'dart:async';
Future? v1;
void f() { Future; }
'''
C['amb/sdk_like.dart'] = '''class Future {}
'''
C['amb/main6.dart'] = '''import 'a.dart' show A;
import 'b.dart' hide A;
A? v1;
void f() {
  foo;
}
'''
C['amb/main7.dart'] = '''import 'a.dart';
import 'b.dart';
class A {}
A? v1;
void f() {
  A;
}
'''
# part
C['part/lib.dart'] = '''part 'naoparte.dart';
part 'lib.dart';
part 'dart:core';
part 'inexistente.dart';
part 'parte_de_outra.dart';
part 'parte.dart';
part 'parte.dart';
'''
C['part/naoparte.dart'] = '''class NP {}
'''
C['part/parte.dart'] = '''part of 'lib.dart';
'''
C['part/parte_de_outra.dart'] = '''part of 'outra.dart';
'''
C['part/outra.dart'] = '''part 'parte_de_outra.dart';
'''
# export
C['exp/a.dart'] = '''main() {}
class A {}
int v = 0;
'''
C['exp/b.dart'] = '''main() {}
class A {}
set v(int _) {}
class OnlyB {}
'''
C['exp/c.dart'] = '''export 'a.dart';
'''
C['exp/main.dart'] = '''export 'a.dart';
export 'b.dart';
'''
C['exp/main2.dart'] = '''export 'a.dart';
export 'c.dart';
'''
C['exp/main3.dart'] = '''export 'a.dart' show A;
export 'b.dart' hide A, main;
'''
C['exp/main4.dart'] = '''export 'a.dart';
export 'b.dart';
export 'b.dart';
class A {}
'''
C['exp/user.dart'] = '''import 'main.dart';
A? x;
OnlyB? y;
void f() { main(); v; v = 1; }
'''
for k, v in C.items():
    p = os.path.join(B, k.replace('/', os.sep))
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, 'w', encoding='utf-8', newline='\n') as f:
        f.write(v)
print(len(C))
