class A {
  int operator[](int index) => 0;

  operator[]=(int index, int value) {}
}

extension E on A? {
  void bar() {
    this[0];
//      ^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '[]' can't be unconditionally invoked because the receiver can be 'null'.
    this?[0];

    this[0] = 0;
//      ^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '[]' can't be unconditionally invoked because the receiver can be 'null'.
    this?[0] = 0;
  }
}
