enum _E {
  v;
  void _foo({int? a}) {}
}

void f() {
  _E.v._foo(a: 0);
}
