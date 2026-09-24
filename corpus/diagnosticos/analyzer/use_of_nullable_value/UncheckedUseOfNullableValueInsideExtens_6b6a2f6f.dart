class A {
  set foo(int _) {}
}

extension E on A? {
  set bar(int _) {}

  void baz() {
    foo = 0;
//  ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
    this.foo = 0;
//       ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
    this?.foo = 0;

    bar = 0;
    this.bar = 0;
    this?.bar = 0;
  }
}
