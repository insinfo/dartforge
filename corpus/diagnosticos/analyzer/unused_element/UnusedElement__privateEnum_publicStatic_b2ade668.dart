enum _E {
  v;
  static set foo(int _) {}
}

void f() {
  _E.v;
  _E.foo = 0;
}
