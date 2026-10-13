mixin M1 {
  num get foo => 0;
}

mixin M2 {
  set foo(int v) {}
}

enum E with M1, M2 {
  v
}
