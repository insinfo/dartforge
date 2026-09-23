void f(void x) {
  -x;
//^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'unary-' can't be unconditionally invoked because the receiver can be 'null'.
// ^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}
