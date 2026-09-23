class A {
  @override
  static int get foo => 0;
//               ^^^
// [diag.overrideOnNonOverridingGetter] The getter doesn't override an inherited getter.
}
