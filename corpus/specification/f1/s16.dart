import 'dart:async' as p;
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
