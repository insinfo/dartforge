class A<T> {
  void f() {
    (T) = 0;
//   ^
// [diag.patternAssignmentNotLocalVariable] Only local variables can be assigned in pattern assignments.
  }
}
