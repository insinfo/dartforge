class A {}
class B extends A {
  operator +(value) {
    return super + value;
//               ^
// [diag.undefinedSuperOperator] The operator '+' isn't defined in a superclass of 'B'.
  }
}
