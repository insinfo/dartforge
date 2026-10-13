class A {
  const A();
  operator ==(other) => false;
}
class B {
  static const a = const A();
}
main() {
  const {B.a};
//       ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
