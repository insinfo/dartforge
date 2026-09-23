enum _E {
  v;
  static void _foo() {}
}

void f() {
  _E.v;
  _E._foo();
}
