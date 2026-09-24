m() {
  int? x;
  x.round();
//  ^^^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'round' can't be unconditionally invoked because the receiver can be 'null'.
}
