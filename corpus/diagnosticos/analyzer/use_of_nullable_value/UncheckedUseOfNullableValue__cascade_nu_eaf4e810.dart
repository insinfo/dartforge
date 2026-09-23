m() {
  int? x;
  x..abs();
//   ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'abs' can't be unconditionally invoked because the receiver can be 'null'.
}
