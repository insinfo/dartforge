extension type E(int it) {
  this : assert(this.hashCode == 0);
//              ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
