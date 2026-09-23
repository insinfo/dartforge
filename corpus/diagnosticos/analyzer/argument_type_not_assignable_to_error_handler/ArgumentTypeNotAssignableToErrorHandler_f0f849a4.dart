void f(Stream<void> stream) {
  stream.listen((_) {}, onError: ({Object a = 1}) {});
//                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function({Object a})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
