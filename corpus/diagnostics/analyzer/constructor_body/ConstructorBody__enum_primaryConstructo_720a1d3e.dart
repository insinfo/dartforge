enum const E() {
  v;
  this => null;
//     ^^
// [diag.constPrimaryConstructorWithExpressionBody] The body part of a constant primary constructor can't have an expression body.
}
