void f() {
  int v;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.

  v = 0;

  void f() {
//     ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
    int v; // 1
    v;
//  ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'v' must be assigned before it can be used.
  }
}
