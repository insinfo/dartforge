mixin M {
  static var f = this;
//               ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
