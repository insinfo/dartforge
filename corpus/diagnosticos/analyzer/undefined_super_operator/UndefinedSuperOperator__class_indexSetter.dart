class A {}
class B extends A {
  operator []=(index, value) {
    super[index] = 0;
//       ^^^^^^^
// [diag.undefinedSuperOperator] The operator '[]=' isn't defined in a superclass of 'B'.
  }
}
