void f() {
  int v;
  v ??= v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
//      ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
//      ^^
// [diag.deadCode] Dead code.
//      ^
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.
}
