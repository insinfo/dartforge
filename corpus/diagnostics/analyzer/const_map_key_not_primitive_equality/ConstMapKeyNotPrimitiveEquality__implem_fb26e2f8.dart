class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (0, const A()): 0,
//^^^^^^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '(int, A)' does.
};
