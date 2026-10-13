class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (0, const A()),
//^^^^^^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type '(int, A)' does.
};
