enum _E {
  v;
  void _foo() {}
}

void f() {
  _E.v._foo();
}
