class A {
  int x = 0;
}
extension E on A {
  static void s() {
    x;
    y;
  }
  void m() {
    x;
    y;
    y = 1;
    y();
  }
}
extension on A? {
  void n() {
    x;
    zz;
  }
}
