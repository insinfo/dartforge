List<U Function<U>(U, int)> foo(T Function<T>(T a) f) {
  return [f];
//        ^
// [diag.listElementTypeNotAssignable] The element type 'T Function<T>(T)' can't be assigned to the list type 'U Function<U>(U, int)'.
}
