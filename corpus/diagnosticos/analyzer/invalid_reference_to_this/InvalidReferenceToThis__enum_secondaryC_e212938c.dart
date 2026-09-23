enum E {
  v.named();
  const E.named() : assert(this.hashCode == 0);
//                         ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
//                         ^^^^^^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
}
