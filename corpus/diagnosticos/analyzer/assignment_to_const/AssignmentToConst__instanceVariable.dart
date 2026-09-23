class A {
  static const v = 0;
}
f() {
  A.v = 1;
//  ^
// [diag.assignmentToConst] Constant variables can't be assigned a value after initialization.
}