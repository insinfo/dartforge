void f() {
  3(5);
//^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
}
