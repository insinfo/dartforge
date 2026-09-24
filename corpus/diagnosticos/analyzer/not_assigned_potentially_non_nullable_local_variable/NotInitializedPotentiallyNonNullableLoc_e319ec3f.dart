void f(bool b) {
  int v;
  while (true) {
    if (b) break;
    v = 0;
    v;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
