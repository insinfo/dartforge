void f(bool b) {
  int v1, v2;
  for (var _ in [0, 1, 2]) {
    v1 = 0;
    if (b) continue;
    v2 = 0;
  }
  v1;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v1' must be assigned before it can be used.
  v2;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
}
