class A {
  int _ = 0;
  int _x = 0;
  void m(int _) { print(_); }
  void n() { var _x = 1; print(_x); }
  A(int _x, {int? y});
  A.o() { A(1, y: 2); }
}
