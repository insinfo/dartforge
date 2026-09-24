final a = 0;

void f(x) {
  if (x case > a) {}
//             ^
// [diag.nonConstantRelationalPatternExpression] The relational pattern expression must be a constant.
}
