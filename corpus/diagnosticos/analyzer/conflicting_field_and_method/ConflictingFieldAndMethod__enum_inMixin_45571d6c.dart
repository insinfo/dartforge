mixin M {
  void foo() {}
}

enum E with M {v}

augment enum E {;
  int get foo => 0;
}
