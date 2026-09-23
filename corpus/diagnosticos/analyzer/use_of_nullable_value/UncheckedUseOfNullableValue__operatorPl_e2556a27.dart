m() {
  int? x;
  x + 3;
//  ^
// [diag.uncheckedOperatorInvocationOfNullableValue] The operator '+' can't be unconditionally invoked because the receiver can be 'null'.
}
