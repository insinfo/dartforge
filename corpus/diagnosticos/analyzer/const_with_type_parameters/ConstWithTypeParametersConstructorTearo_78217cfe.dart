void f<T>(T a) {}
void g() {
  const [f as void Function<T>(T, [int])];
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.listElementTypeNotAssignable] The element type 'void Function<T>(T)' can't be assigned to the list type 'void Function<T>(T, [int])'.
}
