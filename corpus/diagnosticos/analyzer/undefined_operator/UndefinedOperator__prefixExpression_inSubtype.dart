class A {}
class B extends A {
  operator +(B b) {return new B();}
}
f(a) {
  if (a is A) {
    ++a;
//  ^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'A'.
  }
}
