class X {}
class Y {}
class A implements X {}
class A implements Y {
  void m() {
    X x = this;
    Y y = this;
    print([x, y]);
  }
}
void f(A a) {
  X x = a;
  Y y = a;
  print([x, y]);
}
