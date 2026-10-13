m() {
  int? x;
  x.isEven;
//  ^^^^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'isEven' can't be unconditionally accessed because the receiver can be 'null'.
}
