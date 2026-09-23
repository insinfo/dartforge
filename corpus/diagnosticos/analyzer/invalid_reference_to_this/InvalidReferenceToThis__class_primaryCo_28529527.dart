class A(Object x);
class B() extends A {
  this : super(this);
//             ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
