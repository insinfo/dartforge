void f(Never? x) {
  x + (1 + 2);
//  ^
// [diag.uncheckedOperatorInvocationOfNullableValue] The operator '+' can't be unconditionally invoked because the receiver can be 'null'.
}
