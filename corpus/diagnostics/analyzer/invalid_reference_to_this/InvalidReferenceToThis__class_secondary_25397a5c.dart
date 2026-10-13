class A {
  A(x) {}
}
class B extends A {
  B() : super(this);
//            ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
