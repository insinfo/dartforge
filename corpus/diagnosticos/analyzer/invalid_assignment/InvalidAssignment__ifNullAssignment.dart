void f(int i) {
  double? d;
  d ??= i;
//      ^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'double?'.
}
