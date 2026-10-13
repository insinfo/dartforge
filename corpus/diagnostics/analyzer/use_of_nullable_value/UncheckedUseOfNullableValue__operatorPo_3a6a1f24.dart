class A {}

extension E on A {
  A operator +(int _) => this;
}

m(A? x) {
  x++;
// ^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '+' can't be unconditionally invoked because the receiver can be 'null'.
}
