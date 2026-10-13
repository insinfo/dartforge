m<T extends int?>(T x) {
  x.isEven;
//  ^^^^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'isEven' can't be unconditionally accessed because the receiver can be 'null'.
}
