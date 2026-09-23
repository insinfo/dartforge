void f(int x) {
  if (x case var a when () {
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    a = 0;
//  ^
// [diag.patternVariableAssignmentInsideGuard] Pattern variables can't be assigned inside the guard of the enclosing guarded pattern.
    return true;
  }()) {}
}
