const v = {A(): 0};
//         ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.

class A {
  const A();
  int get hashCode => 0;
}
