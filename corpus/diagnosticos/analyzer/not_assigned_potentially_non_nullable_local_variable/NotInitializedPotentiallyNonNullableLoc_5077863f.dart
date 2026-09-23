void f(bool b) {
  int v;
  do {
    if (b) break;
    v = 0;
  } while (b);
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
