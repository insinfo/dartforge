f() {
  const x = 0;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  x = 1;
//^
// [diag.assignmentToConst] Constant variables can't be assigned a value after initialization.
}
