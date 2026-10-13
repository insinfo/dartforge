class A {
  const A();
  operator ==(other) => false;
}

main() {
  const {const A() : 0};
//       ^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
