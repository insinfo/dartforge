void f(e, int a) {
  switch (e) {
    case const (3 + a):
//                  ^
// [diag.constantPatternWithNonConstantExpression] The expression of a constant pattern must be a valid constant.
      break;
  }
}
