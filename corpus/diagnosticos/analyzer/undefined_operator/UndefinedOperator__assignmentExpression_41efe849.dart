class A {}
class B {
  f(A a) {
    A a2 = new A();
    a += a2;
//    ^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'A'.
  }
}
