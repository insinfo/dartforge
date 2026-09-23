extension E on int {
  int get foo => 0;
}

m(int? x) {
  x.foo;
//  ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
}
