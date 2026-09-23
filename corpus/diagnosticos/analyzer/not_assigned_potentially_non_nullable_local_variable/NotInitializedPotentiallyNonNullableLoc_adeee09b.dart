void f() {
  int v1, v2;

  v1 = 0;

  [0, 1, 2].forEach((t) {
    v1;
    v2;
//  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
  });
}
