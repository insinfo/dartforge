f() {
  if ([1, 2, 3]) return 2; else return 1;
//    ^^^^^^^^^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
