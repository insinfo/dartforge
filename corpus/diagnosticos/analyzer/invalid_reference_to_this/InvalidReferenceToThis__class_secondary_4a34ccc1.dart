class A {
  var f;
  A() : f = this;
//          ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
