class A {
  var x = 0;
}

class B extends A {
  void f() {
    (x) = 0;
//   ^
// [diag.patternAssignmentNotLocalVariable] Only local variables can be assigned in pattern assignments.
  }
}
