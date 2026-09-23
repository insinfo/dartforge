f(bool b) {
  int v;
  b ? (v = 1) : 2;
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
