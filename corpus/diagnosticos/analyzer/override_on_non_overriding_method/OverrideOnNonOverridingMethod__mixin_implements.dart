class A {
  void foo() {}
}

mixin M implements A {
  @override
  void foo() {}
}
