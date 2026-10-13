T f<T>(T a) => a;
U Function<U>(U, int) foo = f;
//                          ^
// [diag.invalidAssignment] A value of type 'T Function<T>(T)' can't be assigned to a variable of type 'U Function<U>(U, int)'.
