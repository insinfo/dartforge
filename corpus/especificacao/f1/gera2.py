import os
B = r'E:\dftemp\analise\spec-r4\casos\f1\b2'
C = {}
C['t01.dart'] = '''import 'dart:core' as core;
class A {
  core.List foo = 0;
  set core(_) {}
}
'''
C['t02.dart'] = '''sealed class Option<A> {}

final class None implements Option<A> {
  const None();
}

A doOption<A>(
  A Function(B Function<B>(Option<B>)) eval,
) {
  return eval(
    <B>(option) => switch (option) {
      None() => throw 7,
    },
  );
}
'''
C['t03.dart'] = '''// @dart=2.19
Record? r;
void f() {
  Record;
}
'''
C['t05.dart'] = '''import 'dart:math' as max;
import 'dart:math';
void f() {
  max;
  max(1, 2);
  max.max(1, 2);
  min(1, 2);
}
'''
C['t06.dart'] = '''import 'dart:math' as p;
p.Undef? v;
void f(Object o) {
  new p.Undef();
  const p.Undef();
  new p.Undef.named();
  new p.pi();
  try {} on p.Undef catch (e) {}
  o is p.Undef;
  o as p.Undef;
  <p.Undef>[];
  p.pi x;
}
class C {
  factory C.r() = p.Undef;
  C();
}
class D extends p.Undef {}
'''
C['t07.dart'] = '''import 'dart:foo';

void f() {
  A(foo: 0);
  foo(0, bar: 1);
  E(0).foo();
  A;
  A x;
}
'''
C['t08.dart'] = '''UnknownType getValue() => UnknownType();
class A {
  factory A() {
    foo();
    return throw 0;
  }
  static baz() => X();
  static var f = Y();
  var g = Z();
}
'''
C['t11.dart'] = '''enum E<T> {
  v<T>(),
  w<int>();
  const E();
}
'''
C['t12.dart'] = '''import 'dart:async' as a;
class A {
  get a => 0;
  void m() {
    a.Future;
    a.Future.value(1);
    a.Future? x;
  }
}
void f(int a) {
  a.Future;
  a.Future.value(1);
}
'''
C['t13.dart'] = '''import 'dart:math' as p;
class A<p> {
  p.Random? x;
  void m() {
    p.pi;
    p;
  }
}
void f<p>() {
  p.pi;
}
'''
C['t14.dart'] = '''void f() {
  var x = x;
  int y = y + 1;
  g(g);
  void g(Object o) {}
}
int a = 0;
void h() {
  print(a);
  {
    print(a);
  }
  var a = 1;
}
void k(int a) {
  void inner() { print(b); }
  var b = 0;
  inner();
}
'''
C['t15.dart'] = '''class A {
  static void s() {}
  void m() {}
  int f = 0;
}
class B extends A {
  static void t() {
    m();
    f;
    s();
  }
}
mixin M on A {
  void n() {
    s();
    g();
  }
}
'''
C['t16.dart'] = '''import 'dart:math' as _;
import 'dart:async' as __;
void f() {
  _.pi;
  __.Future;
  _;
}
'''
C['t17.dart'] = '''typedef F = void Function();
typedef G<T> = List<T>;
int v = 0;
void fn() {}
void f(Object o) {
  F();
  G();
  G<int>();
  new F();
  v();
  fn.call();
  dynamic();
  Never();
}
'''
C['t18.dart'] = '''class A {
  int x = 0;
}
extension E on A {
  static void s() {
    x;
    y;
  }
  void m() {
    x;
    y;
    y = 1;
    y();
  }
}
extension on A? {
  void n() {
    x;
    zz;
  }
}
'''
C['exp2/a.dart'] = '''class A {}
main() {}
class Zed {}
'''
C['exp2/b.dart'] = '''class Zed {}
class A {}
main() {}
'''
C['exp2/main.dart'] = '''export 'a.dart';
export 'b.dart';
'''
C['exp2/c.dart'] = '''class OnlyC {}
class A {}
'''
C['exp2/main2.dart'] = '''export 'a.dart' hide A;
export 'c.dart';
export 'b.dart' show A;
'''
C['priv/lib.dart'] = '''class B {
  bool _instanceField = false;
  static int _s = 0;
  void _m() {}
}
int _top = 0;
int pub = 0;
'''
C['priv/main.dart'] = '''import 'lib.dart';
import 'lib.dart' as p;
class Test extends B {
  test() {
    _instanceField = true;
    _instanceField;
    _m();
    _s;
    _top;
    p._top;
    p.pub;
  }
}
'''
for k, v in C.items():
    p = os.path.join(B, k.replace('/', os.sep))
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, 'w', encoding='utf-8', newline='\n') as f:
        f.write(v)
print(len(C))
