enum _E {
  v;
  void foo([int? a]) {}
}

void f() {
  _E.v.foo();
}
