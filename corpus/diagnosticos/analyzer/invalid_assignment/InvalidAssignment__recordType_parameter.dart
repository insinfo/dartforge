void f((int a, int b) r) {
  r = (a: 1, b: 2);
//    ^^^^^^^^^^^^
// [diag.invalidAssignment] A value of type '({int a, int b})' can't be assigned to a variable of type '(int, int)'.
}
