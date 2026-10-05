class A {
  int _a = 0;
  int _b = 0;
  void _m() {}
  void _n() {}
}
class B extends A {
  void x() {
    print(super._a);
    super._m();
  }
}
/// [A._b] e [A._n] só em comentário.
void f() {}
