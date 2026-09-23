class A {
  const A();

  bool operator ==(other) => false;
}

class B {
  const B(_);
}

main() {
  const B({A(): 0});
//         ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
