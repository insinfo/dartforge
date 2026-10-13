void f() {
  int v;
  (v = 0) ?? 0;
//        ^^^^
// [diag.deadCode] Dead code.
//           ^
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.
  v;
}
