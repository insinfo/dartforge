f(int x) {
  x ??= 0;
//      ^^
// [diag.deadCode] Dead code.
//      ^
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.
}
