class A {}
class B extends A {
  operator [](index) {
    return super[index + 1];
//              ^^^^^^^^^^^
// [diag.undefinedSuperOperator] The operator '[]' isn't defined in a superclass of 'B'.
  }
}
