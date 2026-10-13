class A {
  A(Object x);
  A.named() : this(this);
//                 ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
