enum E {
  v.named();
  const E.named() : this(this);
//                       ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
// [diag.invalidConstant] Invalid constant value.
  const E(Object o);
}
