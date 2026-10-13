enum E {
  v;
  static set _foo(int _) {}
}

void f() {
  E._foo = 0;
}
