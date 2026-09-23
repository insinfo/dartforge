void f(int x) {
  if (x case var a when (a = 1) > 0) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                       ^
// [diag.patternVariableAssignmentInsideGuard] Pattern variables can't be assigned inside the guard of the enclosing guarded pattern.
}
