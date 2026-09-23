class A {
  int x;
  A(this.x);
}

class B {
  final A? a;
  B(this.a);
}

m(B b) {
  b.a?.x = 1;
  b.a.x = 2;
//    ^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'x' can't be unconditionally accessed because the receiver can be 'null'.
}
