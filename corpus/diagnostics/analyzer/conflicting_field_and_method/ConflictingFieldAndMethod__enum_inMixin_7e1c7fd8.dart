mixin M {
  void foo() {}
}

enum E {
  v;
  int get foo => 0;
}

augment enum E with M {}
