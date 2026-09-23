void f() {
  (dynamic) = 0;
// ^^^^^^^
// [diag.patternAssignmentNotLocalVariable] Only local variables can be assigned in pattern assignments.
}
