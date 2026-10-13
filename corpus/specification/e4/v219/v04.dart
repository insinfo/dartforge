// @dart=2.12
class A<T> { const A(); }
@A<int>() class C {}
@A<int> class D {}
