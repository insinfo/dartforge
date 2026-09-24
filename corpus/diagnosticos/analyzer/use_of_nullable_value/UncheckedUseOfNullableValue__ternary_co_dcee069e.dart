m() {
  bool? x;
  x ? 0 : 1;
//^
// [diag.uncheckedUseOfNullableValueAsCondition] A nullable expression can't be used as a condition.
}
