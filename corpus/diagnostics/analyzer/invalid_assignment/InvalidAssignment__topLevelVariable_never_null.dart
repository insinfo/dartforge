Never x = throw 0;

void f() {
  x = null;
//    ^^^^
// [diag.invalidAssignment] A value of type 'Null' can't be assigned to a variable of type 'Never'.
}
