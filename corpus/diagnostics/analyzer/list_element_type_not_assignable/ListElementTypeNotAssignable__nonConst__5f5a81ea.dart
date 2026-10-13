List<int Function(int, int)> foo(T Function<T>(T a) f) {
  return [f];
//        ^
// [diag.listElementTypeNotAssignable] The element type 'dynamic Function(dynamic)' can't be assigned to the list type 'int Function(int, int)'.
}
