class A() {
  var f;
  this : f = this;
//           ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
