void f(int? x) {
  if (x case > 0) {}
//           ^
// [diag.uncheckedOperatorInvocationOfNullableValue] The operator '>' can't be unconditionally invoked because the receiver can be 'null'.
}
