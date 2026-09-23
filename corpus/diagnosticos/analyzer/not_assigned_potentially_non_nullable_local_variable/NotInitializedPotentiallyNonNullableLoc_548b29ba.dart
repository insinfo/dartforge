void f() {
  int v1, v2;
  do {
    v1; // assigned in the condition, but not yet
//  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v1' must be assigned before it can be used.
  } while ((v1 = 0) + (v2 = 0) >= 0);
  v2;
}
