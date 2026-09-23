mixin M on Enum {
  void foo() {
    super.index;
  }
}

enum E with M {
  v
}
