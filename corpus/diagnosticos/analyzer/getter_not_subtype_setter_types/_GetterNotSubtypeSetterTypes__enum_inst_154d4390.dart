mixin M {
  num get foo => 0;
}

enum E with M {
  v;
  set foo(int v) {}
}
