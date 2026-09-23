class A {}

class B extends A {
  @override
  void foo() {}
//     ^^^
// [diag.overrideOnNonOverridingMethod] The method doesn't override an inherited method.
}
