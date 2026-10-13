enum _E {
  v;
  static void foo() {}
}

void f() {
  _E.v;
  _E.foo();
}
