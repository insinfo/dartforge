enum _E {
  v;
  set _foo(int _) {}
}

void f() {
  _E.v._foo = 0;
}
