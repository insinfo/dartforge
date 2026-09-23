void f() {
  int v;
  while (true) {
    // No assignment, but no break.
    // So, we don't exit the loop.
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
//^^
// [diag.deadCode] Dead code.
}
