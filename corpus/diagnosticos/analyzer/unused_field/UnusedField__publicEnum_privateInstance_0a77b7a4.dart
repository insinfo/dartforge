enum E {
  v;
  final int _foo = 0;
}

void f() {
  E.v._foo;
}
