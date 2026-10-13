class A {
  void foo() {}
}

extension E on A? {
  void bar() {
    foo();
//  ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'foo' can't be unconditionally invoked because the receiver can be 'null'.
    this.foo();
//       ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'foo' can't be unconditionally invoked because the receiver can be 'null'.
    this?.foo();

    bar();
    this.bar();
    this?.bar();
  }
}
