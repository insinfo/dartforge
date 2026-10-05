class B { const B(int x) : assert(x > 0); }
class A { const A(Object o); }
const a = const A(const B(0));
const b = const A(const A('a'.foo));
var c = const A(const B(0));
