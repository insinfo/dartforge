class A {
  const A();
  operator ==(other) => false;
}

class B {
  static const a = const A();
}

main() {
  const {B.a : 0};
//       ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
