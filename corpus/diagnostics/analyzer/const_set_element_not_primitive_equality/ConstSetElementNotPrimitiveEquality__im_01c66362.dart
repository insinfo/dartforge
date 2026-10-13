class A {
  const A();
  operator ==(other) => false;
}
class B extends A {
  const B();
}
main() {
  const {const B()};
//       ^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'B' does.
}
