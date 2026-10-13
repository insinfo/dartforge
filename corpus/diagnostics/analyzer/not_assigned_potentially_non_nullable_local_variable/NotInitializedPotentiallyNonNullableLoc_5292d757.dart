void f() {
  int v;
  try {
    v = 0;
  } catch (_) {
    // not assigned
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
