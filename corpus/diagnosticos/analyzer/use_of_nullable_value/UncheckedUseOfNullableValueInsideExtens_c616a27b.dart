class A {
  int get foo => 0;
}

extension E on A? {
  int get bar => 0;

  void baz() {
    foo;
//  ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
    this.foo;
//       ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
    this?.foo;

    bar;
    this.bar;
    this?.bar;
  }
}
