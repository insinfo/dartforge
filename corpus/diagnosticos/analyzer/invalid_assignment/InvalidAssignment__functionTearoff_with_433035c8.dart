void f<T>(T a) {}

void Function(String) g = f<int>;
//                        ^^^^^^
// [diag.invalidAssignment] A value of type 'void Function(int)' can't be assigned to a variable of type 'void Function(String)'.
