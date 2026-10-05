class X {}
class Y {}
class A implements X {
  void m() {
    X x = this;
    Y y = this;
    print([x, y]);
  }
}
class A implements Y {}
void f(A a) {
  X x = a;
  Y y = a;
  print([x, y]);
}
