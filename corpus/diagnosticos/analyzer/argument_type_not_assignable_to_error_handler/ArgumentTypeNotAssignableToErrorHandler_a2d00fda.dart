void f(Stream<void> stream) {
  stream.listen((_) {}, onError: (Object a, {StackTrace? b}) {});
//                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(Object, {StackTrace? b})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
