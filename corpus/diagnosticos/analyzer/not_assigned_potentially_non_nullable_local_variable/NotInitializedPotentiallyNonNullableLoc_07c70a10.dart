void f() {
  List<int> v1;
  int v2;
  for (var _ in (v1 = [0, 1, 2])) {
    v2 = 0;
  }
  v1;
  v2;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
}
