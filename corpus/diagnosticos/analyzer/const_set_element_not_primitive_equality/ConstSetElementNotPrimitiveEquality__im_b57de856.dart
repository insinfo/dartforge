class A {
  const A();
  operator ==(other) => false;
}
main() {
  const {const A()};
//       ^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
