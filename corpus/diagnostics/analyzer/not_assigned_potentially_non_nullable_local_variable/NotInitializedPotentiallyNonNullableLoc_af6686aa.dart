// %before-language-feature: patterns
void f(bool b, int e) {
  int v1, v2;
  switch (e) {
    case 1:
      v1 = 0;
      if (b) break;
      v2 = 0;
      break;
    default:
      v1 = 0;
      if (b) break;
      v2 = 0;
  }
  v1;
  v2;
//^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
}
