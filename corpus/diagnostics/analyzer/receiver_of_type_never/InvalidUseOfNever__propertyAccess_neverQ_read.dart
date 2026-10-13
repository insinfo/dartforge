void f(Never? x) {
  x.foo;
//  ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
}
