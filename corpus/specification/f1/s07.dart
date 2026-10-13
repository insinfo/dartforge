class A {
  static int s = 0;
  static void sm() {}
}
class B extends A {
  void m() {
    s;
    sm();
    s = 1;
    sm;
  }
}
extension E on A {
  void m() {
    s;
    sm();
  }
}
