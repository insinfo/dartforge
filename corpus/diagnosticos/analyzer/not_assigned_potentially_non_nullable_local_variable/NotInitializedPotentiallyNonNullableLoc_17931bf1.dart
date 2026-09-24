void f(int a) {
  int v;
  a ?? (v = 0);
//  ^^^^^^^^^^
// [diag.deadCode] Dead code.
//     ^^^^^^^
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
