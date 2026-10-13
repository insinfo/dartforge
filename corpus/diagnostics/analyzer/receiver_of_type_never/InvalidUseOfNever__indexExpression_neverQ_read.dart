void f(Never? x) {
  x[0];
// ^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '[]' can't be unconditionally invoked because the receiver can be 'null'.
}
