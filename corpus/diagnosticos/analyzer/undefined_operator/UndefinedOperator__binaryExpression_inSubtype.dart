class A {}
class B extends A {
  operator +(B b) {}
}
f(a) {
  if (a is A) {
    a + 1;
//    ^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'A'.
  }
}
