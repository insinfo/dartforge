enum E() {
  v;
  final Object f;
  this : f = this;
//           ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
// [diag.invalidConstant] Invalid constant value.
}
