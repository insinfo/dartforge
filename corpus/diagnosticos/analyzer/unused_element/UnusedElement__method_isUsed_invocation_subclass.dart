class A {
  _m() {}
}
class B extends A {
  _m() {}
}
void f(A a) {
  a._m();
}
