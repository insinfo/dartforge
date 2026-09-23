m() {
  bool? x;
  if(!x) {}
//    ^
// [diag.uncheckedUseOfNullableValueAsCondition] A nullable expression can't be used as a condition.
}
