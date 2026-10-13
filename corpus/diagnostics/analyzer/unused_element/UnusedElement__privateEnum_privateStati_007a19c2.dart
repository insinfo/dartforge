enum _E {
  v;
  static set _foo(int _) {}
}

void f() {
  _E.v;
  _E._foo = 0;
}
