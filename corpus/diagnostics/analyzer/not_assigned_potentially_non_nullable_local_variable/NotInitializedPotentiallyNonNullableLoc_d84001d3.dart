void f(bool b) {
  int v;
  while (b) {
    v = 0;
    v;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
