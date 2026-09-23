void f(int x) {
  if (x case var a when (a += 1) > 0) {}
//                       ^
// [diag.patternVariableAssignmentInsideGuard] Pattern variables can't be assigned inside the guard of the enclosing guarded pattern.
}
