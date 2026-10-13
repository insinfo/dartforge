const v = {A()};
//         ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.

class A {
  const A();
  int get hashCode => 0;
}
