void f(bool c) {
  int v;
  if (c) {
    // not assigned
  } else {
    v = 0;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
