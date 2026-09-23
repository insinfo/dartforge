class A {}

mixin M on A {
  @override
  void foo() {}
//     ^^^
// [diag.overrideOnNonOverridingMethod] The method doesn't override an inherited method.
}
