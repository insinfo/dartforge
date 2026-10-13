class A {
  static void s() {}
  void m() {}
  int f = 0;
}
class B extends A {
  static void t() {
    m();
    f;
    s();
  }
}
mixin M on A {
  void n() {
    s();
    g();
  }
}
