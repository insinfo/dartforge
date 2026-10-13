void f() {
  int v;
  try {
    // not assigned
  } catch (_) {
    v = 0;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
