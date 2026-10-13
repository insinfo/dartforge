enum E {
  v;
  @override
  final int foo = 0;
//          ^^^
// [diag.overrideOnNonOverridingField] The field doesn't override an inherited getter or setter.
}
