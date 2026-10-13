void f(Null a) {
  a ? 0 : 1;
//^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
