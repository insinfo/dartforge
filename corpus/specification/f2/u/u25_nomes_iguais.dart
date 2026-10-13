class A {
  void _m() {}
  int _f = 0;
}
class B {
  void _m() {}
  int _f = 0;
}
void f(B b) {
  b._m();
  print(b._f);
}
