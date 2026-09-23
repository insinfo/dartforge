void f() {
  int v1, v2;

  v1 = 0;

  void f() {
//     ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
    v1;
    v2;
//  ^^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v2' must be assigned before it can be used.
  }

  v2 = 0;
}
