void f() {
  int v;

  [0, 1, 2].forEach((t) {
    v = t;
  });

  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
