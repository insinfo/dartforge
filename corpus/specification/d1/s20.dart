class A { const A(Object o); }
@A(v)
class C {}
var v = 0;
enum E { v(a); const E(Object o); }
var a = 1;
@A(foo)
class D<@A(foo) T> {}
