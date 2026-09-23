void f(bool c) {
  int v;
  c || ((v = 0) >= 0);
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
