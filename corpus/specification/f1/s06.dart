class A {
  int x = 0;
  void im() {}
  static void s() {
    x;
    im();
    x = 1;
  }
  factory A.f() {
    x;
    return throw 0;
  }
  A() : y = x;
  int y;
}
