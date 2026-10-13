void f() {
  int v1;

  v1 = 0;

  void f() {
//     ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
    int v2, v3;
    v2 = 0;
    v1;
    v2;
    v3;
//  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v3' must be assigned before it can be used.
  }
}
