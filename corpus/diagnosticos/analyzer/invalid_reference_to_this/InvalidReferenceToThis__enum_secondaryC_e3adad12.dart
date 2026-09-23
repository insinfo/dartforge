enum E {
  v;
  final Object f;
  const E() : f = this;
//                ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
// [diag.invalidConstant] Invalid constant value.
}
