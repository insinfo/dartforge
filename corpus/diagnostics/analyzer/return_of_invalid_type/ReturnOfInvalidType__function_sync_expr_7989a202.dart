U Function<U>(U, int) foo(T Function<T>(T a) f) => f;
//                                                 ^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'T Function<T>(T)' can't be returned from the function 'foo' because it has a return type of 'U Function<U>(U, int)'.
