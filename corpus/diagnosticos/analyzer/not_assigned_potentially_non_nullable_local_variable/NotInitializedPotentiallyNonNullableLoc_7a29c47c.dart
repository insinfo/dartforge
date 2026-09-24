void f(bool b) {
  int v1, v2, v3, v4;
//            ^^
// [diag.unusedLocalVariable] The value of the local variable 'v3' isn't used.
  for (; b; v1 = 0, v2 = 0, v3 = 0, v4) {
//                                  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v4' must be assigned before it can be used.
    v1;
//  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v1' must be assigned before it can be used.
  }
  v2;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
}
