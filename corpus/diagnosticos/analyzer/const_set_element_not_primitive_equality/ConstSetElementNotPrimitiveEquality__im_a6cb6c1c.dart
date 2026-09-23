class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (a: 0, b: const A()),
//^^^^^^^^^^^^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type '({int a, A b})' does.
};
