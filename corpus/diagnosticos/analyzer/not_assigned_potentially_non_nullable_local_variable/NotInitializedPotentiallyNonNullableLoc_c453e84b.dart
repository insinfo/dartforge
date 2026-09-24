void f() {
  List<int> v;
  v[0] = (v = [1, 2])[1];
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
  v;
}
