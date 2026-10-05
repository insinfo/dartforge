class A {
  int get foo => 0;
  static int get sfoo => 0;
}
class B extends A {
  set foo(int _) {}
  set sfoo(int _) {}
  void m() {
    foo;
    sfoo;
  }
}
int get top => 0;
class C {
  set top(int _) {}
  static set stop(int _) {}
  void m() {
    top;
    stop;
    top = 1;
  }
  static void s() {
    top;
  }
}
