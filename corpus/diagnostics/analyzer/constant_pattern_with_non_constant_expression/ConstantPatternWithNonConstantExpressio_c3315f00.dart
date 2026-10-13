class A {
  static final a = 0;
}

void f(x) {
  if (x case A.a) {}
//           ^^^
// [diag.constantPatternWithNonConstantExpression] The expression of a constant pattern must be a valid constant.
}
