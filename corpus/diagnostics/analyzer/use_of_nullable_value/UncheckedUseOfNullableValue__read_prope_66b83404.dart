class A {
  final int x;
  A(this.x);
}

class B {
  final A a;
  B(this.a);
}

class C {
  final B? b;
  C(this.b);
}

m(C c) {
  c.b?.a.x; // 1
  c.b.a.x; // 2
//    ^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'a' can't be unconditionally accessed because the receiver can be 'null'.
}
