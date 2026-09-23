extension E on int {
  static var f = this;
//               ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
