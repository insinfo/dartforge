import 'dart:math' deferred as m;
class A {
  final Object x;
  static int s = 0;
  static const c = 0;
  static int sm() => 0;
  int im() => 0;
  const A.a(int p) : x = s;
  const A.b(int p) : x = c;
  const A.c(int p) : x = sm;
  const A.d(int p) : x = A.sm;
  const A.e(int p) : x = identical(p, s);
  const A.f(int p) : x = p > 0 ? -p : ~p;
  const A.g(int p) : x = p++;
  const A.h(int p) : x = m.pi;
  const A.i(int p) : x = (p, a: s);
  const A.j(int p) : x = 'a' 'b${s}';
  const A.k(int p) : x = #sym;
  const A.l(int p) : x = () => p;
  const A.m(int p) : x = p.toString();
  const A.n(int p) : x = this;
  const A.o(String p) : x = p.length;
  const A.p(String p) : x = p.isEmpty;
  const A.q(int p) : x = A.s;
  const A.r(int p) : x = p ?? s;
  const A.t(int p) : x = switch (p) { _ => 0 };
  const A.u(int p) : x = [p][0];
  const A.v(int p) : x = p == 0 ? null : throw p;
  const A.w(int p) : x = x2;
  const A.y(int p) : x = await p;
  void mm(int q) { const A.a(q); }
}
const x2 = 0;
