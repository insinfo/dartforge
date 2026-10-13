f(Unresolved o) {
//^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
  int? i = o.nonNull;
  i.isEven;
//  ^^^^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'isEven' can't be unconditionally accessed because the receiver can be 'null'.
}
