class A {
  set foo(int _) {}
  static set bar(int _) {}
  void m() {
    foo;
    bar;
    foo();
    bar();
  }
}
