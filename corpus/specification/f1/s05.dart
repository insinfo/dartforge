void f() {
  g();
  g2<int>(1);
}
class A {
  void m() {
    g();
  }
  static void s() {
    g();
  }
}
extension E on int {
  void m() {
    g();
  }
}
var v = g();
