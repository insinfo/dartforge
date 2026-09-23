void f(Null a) {
  if (a) {}
//    ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
