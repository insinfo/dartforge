void f(int x) {
  if (x case var a when () {
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    if (x case _ when (a = 1) > 0) {}
//                     ^
// [diag.patternVariableAssignmentInsideGuard] Pattern variables can't be assigned inside the guard of the enclosing guarded pattern.
    return true;
  }()) {}
}
