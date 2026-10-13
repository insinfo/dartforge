extension E on int {
  @override
  void foo() {}
//     ^^^
// [diag.overrideOnNonOverridingMethod] The method doesn't override an inherited method.
}
