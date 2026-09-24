void f(Stream<void> stream) {
  stream.listen((_) {}, onError: (String a) {});
//                      ^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(String)' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
