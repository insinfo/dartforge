m() {
  bool? x;
  if(x || false) {}
//   ^
// [diag.uncheckedUseOfNullableValueAsCondition] A nullable expression can't be used as a condition.
}
