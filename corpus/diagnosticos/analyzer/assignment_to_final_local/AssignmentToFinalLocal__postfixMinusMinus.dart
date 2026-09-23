f() {
  final x = 0;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  x--;
//^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
}