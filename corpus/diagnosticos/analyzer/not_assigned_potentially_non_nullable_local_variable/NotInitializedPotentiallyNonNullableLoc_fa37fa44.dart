void f() {
  int v;

  void f() {
//     ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
    v = 0;
  }

  v;
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
}
