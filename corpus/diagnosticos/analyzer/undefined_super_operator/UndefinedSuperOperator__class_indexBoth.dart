class A {}
class B extends A {
  operator [](index) {
    return super[index]++;
//              ^^^^^^^
// [diag.undefinedSuperOperator] The operator '[]' isn't defined in a superclass of 'B'.
// [diag.undefinedSuperOperator] The operator '[]=' isn't defined in a superclass of 'B'.
  }
}
