void f(bool b) {
  int v1, v2;
  for (; b; v1 + v2) {
//               ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
    v1 = 0;
    if (b) continue;
    v2 = 0;
  }
}
