mixin M1 {
  void foo() {}
}

mixin M2 on M1 {
  void bar() {
    super.foo();
  }
}

enum E with M1, M2 {
  v
}
