enum _E {
  v;
  static int get _foo => 0;
}

void f() {
  _E.v;
  _E._foo;
}
