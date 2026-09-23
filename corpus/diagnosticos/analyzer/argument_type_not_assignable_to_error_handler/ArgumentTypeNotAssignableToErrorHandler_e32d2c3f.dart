void f(Future<void> future) {
  future.then((_) {}, onError: ({Object a = 1}) {});
//                             ^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function({Object a})' can't be assigned to the parameter type 'FutureOr<Null> Function(Object)' or 'FutureOr<Null> Function(Object, StackTrace)'.
}
