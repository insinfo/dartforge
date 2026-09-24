void f(bool b1, b2) {
  int v1, v2, v3, v4, v5, v6;
  do {
    v1 = 0; // visible outside, visible to the condition
    if (b1) break;
    v2 = 0; // not visible outside, visible to the condition
    v3 = 0; // not visible outside, visible to the condition
    if (b2) continue;
    v4 = 0; // not visible
    v5 = 0; // not visible
  } while ((v6 = v1 + v2 + v4) == 0); // has break => v6 is not visible outside
//                         ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v4' must be assigned before it can be used.
  v1;
  v3;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v3' must be assigned before it can be used.
  v5;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v5' must be assigned before it can be used.
  v6;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v6' must be assigned before it can be used.
}
