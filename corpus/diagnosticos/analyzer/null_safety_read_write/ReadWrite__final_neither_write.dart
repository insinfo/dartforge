void f(bool b) {
  // ignore:unused_local_variable
  final x;
  if (b) x = 0;
  x = 1;
//^
// [diag.assignmentToFinalLocal] The final variable 'x' can only be set once.
}
