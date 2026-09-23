void f(Future<void> future) {
  future.then((_) {}, onError: (Object a, {StackTrace? b}) {});
//                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(Object, {StackTrace? b})' can't be assigned to the parameter type 'FutureOr<Null> Function(Object)' or 'FutureOr<Null> Function(Object, StackTrace)'.
}
