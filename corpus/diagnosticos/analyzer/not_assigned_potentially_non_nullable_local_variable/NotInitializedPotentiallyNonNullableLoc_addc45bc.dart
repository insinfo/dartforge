void f(bool b, int i) {
  int v;
  if (b && (v = i) > 0) {
    v;
  } else {
    v;
//  ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
