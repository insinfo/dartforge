f() {
  if (3) return 2; else return 1;
//    ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
