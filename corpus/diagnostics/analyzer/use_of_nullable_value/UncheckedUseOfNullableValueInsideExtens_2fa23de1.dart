class A {
  A operator-() => this;
}

extension E on A? {
  void bar() {
    -this;
//  ^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'unary-' can't be unconditionally invoked because the receiver can be 'null'.
  }
}
