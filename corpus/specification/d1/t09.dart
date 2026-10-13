class A {
  const A() : assert(1 > 2);
}
class B extends A {
  const B();
}
const b = const B();
class P { final int x; const P(Object o) : x = o.foo; }
class Q extends P { const Q() : super(1); }
const q = const Q();
