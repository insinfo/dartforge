class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (a: 0, b: const A()): 0,
//^^^^^^^^^^^^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '({int a, A b})' does.
};
