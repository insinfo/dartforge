class A {
  A self() => this;
}
class A {
  A self() => this;
  void g(A a) {}
  void h() {
    g(this);
    A x = A();
    print(x is A);
  }
}
