class A {
  var x = 0;

  void f() {
    (x) = 0;
//   ^
// [diag.patternAssignmentNotLocalVariable] Only local variables can be assigned in pattern assignments.
  }
}
