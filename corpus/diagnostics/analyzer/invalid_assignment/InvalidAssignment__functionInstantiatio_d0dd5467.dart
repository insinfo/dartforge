T f<T>(T a) => a;
int Function(int, int) foo = f;
//                           ^
// [diag.invalidAssignment] A value of type 'dynamic Function(dynamic)' can't be assigned to a variable of type 'int Function(int, int)'.
