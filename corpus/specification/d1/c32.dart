class A { const A(); }
@A.x()
class B {}
var v = 1;
class Ann { const Ann(Object o); Ann.nc(); }
@Ann(v)
class C {}
@Ann.nc()
class D {}
@Ann(1 ~/ 0)
class E {}
@v
class F {}
