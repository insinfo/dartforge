class A {
  const A();
  bool operator ==(other) => false;
}

const a = A();
const b = a == 0;
//        ^^^^^^
// [diag.constEvalPrimitiveEquality] In constant expressions, operands of the equality operator must have primitive equality.
