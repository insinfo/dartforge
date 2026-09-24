class A {
  const A();
  operator ==(other) => false;
}

class B extends A {
  const B();
}

main() {
  const {const B() : 0};
//       ^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'B' does.
}
