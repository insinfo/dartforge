class A { const A(); }
class M { }
mixin N { final int f = 0; }
class B = A with M;
class C = A with N;
const b = const B();
const c = const C();
