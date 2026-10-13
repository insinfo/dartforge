enum _E {
  v;
  int get _foo => 0;
}

void f() {
  _E.v._foo;
}
