class A { static const a = B.b; }
class B { static const b = C.c; }
class C { static const c = A.a; }
const x = A.a;
const y = [x];
var z = const [x];
