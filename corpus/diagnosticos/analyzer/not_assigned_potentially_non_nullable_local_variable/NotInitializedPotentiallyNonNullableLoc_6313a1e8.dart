void f(int e) {
  int v;
  switch (e) {
    case 1:
      v = 0;
      break;
  }
  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
