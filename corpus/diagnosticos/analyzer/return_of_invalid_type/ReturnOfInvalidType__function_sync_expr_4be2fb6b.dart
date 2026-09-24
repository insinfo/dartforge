int Function(int, int) foo(T Function<T>(T a) f) => f;
//                                                  ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'dynamic Function(dynamic)' can't be returned from the function 'foo' because it has a return type of 'int Function(int, int)'.
