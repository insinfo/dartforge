class A {
  void foo() {}
}

mixin M on A {
  @override
  void foo() {}
}
