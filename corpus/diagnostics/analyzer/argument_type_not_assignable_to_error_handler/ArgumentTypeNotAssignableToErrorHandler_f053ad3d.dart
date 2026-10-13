void f(Future<void> future) {
  future.then((_) {}, onError: (String a) {});
//                             ^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(String)' can't be assigned to the parameter type 'FutureOr<Null> Function(Object)' or 'FutureOr<Null> Function(Object, StackTrace)'.
}
