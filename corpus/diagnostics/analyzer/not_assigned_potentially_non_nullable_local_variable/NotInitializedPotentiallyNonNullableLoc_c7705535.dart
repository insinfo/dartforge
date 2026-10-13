void f(bool b) {
  int v1, v2, v3;
  L1: do {
    do {
      v1 = 0;
      if (b) break L1;
      v2 = 0;
      v3 = 0;
    } while (b);
    v2;
  } while (b);
  v1;
  v3;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v3' must be assigned before it can be used.
}
