class A {
  @override
  static int foo = 1;
//           ^^^
// [diag.overrideOnNonOverridingField] The field doesn't override an inherited getter or setter.
}
