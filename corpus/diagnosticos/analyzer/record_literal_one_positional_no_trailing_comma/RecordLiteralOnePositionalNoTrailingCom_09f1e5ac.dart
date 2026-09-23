void f((int,) r) {
  r = 1;
//    ^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type '(int,)'.
}
