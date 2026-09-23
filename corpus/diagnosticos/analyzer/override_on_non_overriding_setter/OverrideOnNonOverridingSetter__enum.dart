enum E {
  v;
  @override
  set foo(int _) {}
//    ^^^
// [diag.overrideOnNonOverridingSetter] The setter doesn't override an inherited setter.
}
