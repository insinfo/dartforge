class A {
  final int x;
  A(this.x);
}

m(A? a) {
  a?.x; // 1
  a.x; // 2
//  ^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'x' can't be unconditionally accessed because the receiver can be 'null'.
}
