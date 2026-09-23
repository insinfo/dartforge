void f(int a) {
  // ignore:unused_local_variable
  String v = (a);
//            ^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'String'.
}
