class A {}

extension E on A {
  A operator -() => this;
}

m(A? x) {
  -x;
//^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'unary-' can't be unconditionally invoked because the receiver can be 'null'.
}
