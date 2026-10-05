class A {
  void _m() {}
  void _n() {}
  int get _g => 0;
}
class B extends A {
  @override
  void _m() {}
  @override
  void _n() {}
  @override
  int get _g => 1;
}
class C extends B {
  @override
  void _m() {}
}
void f(A a) {
  a._m();
  print(a._g);
}
