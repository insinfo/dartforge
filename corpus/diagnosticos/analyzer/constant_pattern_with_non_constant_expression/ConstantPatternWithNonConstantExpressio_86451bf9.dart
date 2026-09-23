void f(x) {
  final a = 0;
  if (x case const {0: a}) {}
//                     ^
// [diag.constantPatternWithNonConstantExpression] The expression of a constant pattern must be a valid constant.
}
