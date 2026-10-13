void f(bool b) {
  int v1, v2, v3, v4;
  do {
    v1 = 0; // visible outside, visible to the condition
    if (b) continue;
    v2 = 0; // not visible
    v3 = 0; // not visible
  } while ((v4 = v1 + v2) == 0); // no break => v4 visible outside
//                    ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
  v1;
  v3;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v3' must be assigned before it can be used.
  v4;
}
