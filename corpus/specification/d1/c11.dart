class A {
  const A() : assert(false);
}
class B {
  const B(int x) : assert(x > 0, 'must be positive');
}
class C {
  const C(int x) : assert(x > 0, x);
}
const a = const A();
const b = const B(0);
const c = const C(0);
var d = const B(-1);
