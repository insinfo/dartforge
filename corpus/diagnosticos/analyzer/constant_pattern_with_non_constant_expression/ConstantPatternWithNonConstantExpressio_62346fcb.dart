void f(x) {
  var a = 0;
  if (x case a--) {}
//           ^^^
// [diag.constantPatternWithNonConstantExpression] The expression of a constant pattern must be a valid constant.
}
