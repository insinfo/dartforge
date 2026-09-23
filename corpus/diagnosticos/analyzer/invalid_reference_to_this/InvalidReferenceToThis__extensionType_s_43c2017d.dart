extension type E(int it) {
  static var f = this;
//               ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
