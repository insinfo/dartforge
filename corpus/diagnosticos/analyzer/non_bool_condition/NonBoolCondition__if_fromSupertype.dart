f(Object o) {
  if (o) return 2; else return 1;
//    ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
