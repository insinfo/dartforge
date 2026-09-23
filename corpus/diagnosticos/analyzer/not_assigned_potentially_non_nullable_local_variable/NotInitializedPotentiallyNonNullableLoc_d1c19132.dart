void f(bool b) {
  int v;
  do {
    if (b) break;
  } while ((v = 0) >= 0);
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
