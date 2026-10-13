extension E on int {
  @override
  int get foo => 1;
//        ^^^
// [diag.overrideOnNonOverridingGetter] The getter doesn't override an inherited getter.
}
