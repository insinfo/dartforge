enum _E {
  v;
  static int get foo => 0;
}

void f() {
  _E.v;
  _E.foo;
}
