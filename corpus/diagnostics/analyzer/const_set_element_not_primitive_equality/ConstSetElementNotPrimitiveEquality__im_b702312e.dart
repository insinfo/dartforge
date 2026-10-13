class A {
  const A();

  bool operator ==(other) => false;
}

class B {
  const B(_);
}

main() {
  const B({A()});
//         ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
