class A(int a) {
  this : assert(this.hashCode == 0);
//              ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
