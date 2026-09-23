void f(Never x) {
  x = null;
//    ^^^^
// [diag.invalidAssignment] A value of type 'Null' can't be assigned to a variable of type 'Never'.
}
