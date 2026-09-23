void f(x) {
  final a = 0;
  if (x case const {a: 1}) {}
//                  ^
// [diag.constantPatternWithNonConstantExpression] The expression of a constant pattern must be a valid constant.
}
