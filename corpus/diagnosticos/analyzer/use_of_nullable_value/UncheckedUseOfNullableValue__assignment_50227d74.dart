class A {
  int x;
  int? y;
  A(this.x);
}

class B {
  final A a;
  B(this.a);
}

m(B b) {
  b.a.x += 0;
  b.a.y += 0;
//      ^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '+' can't be unconditionally invoked because the receiver can be 'null'.
}
