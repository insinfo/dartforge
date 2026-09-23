void f(bool b) {
  int v;
  while (true) {
    if (b) continue;
    v = 0;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
//^^
// [diag.deadCode] Dead code.
}
