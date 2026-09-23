void f(x, int a) {
  if (x case > a) {}
//             ^
// [diag.nonConstantRelationalPatternExpression] The relational pattern expression must be a constant.
}
