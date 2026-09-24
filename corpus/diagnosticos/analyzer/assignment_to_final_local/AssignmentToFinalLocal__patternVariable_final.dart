void f() {
  final (a) = 0;
  a = 1;
//^
// [diag.assignmentToFinalLocal] The final variable 'a' can only be set once.
  a;
}
