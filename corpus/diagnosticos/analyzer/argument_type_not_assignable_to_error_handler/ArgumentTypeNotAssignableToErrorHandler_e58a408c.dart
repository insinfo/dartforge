void f(Future<void> future) {
  future.then((_) {}, onError: () {});
//                             ^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function()' can't be assigned to the parameter type 'FutureOr<Null> Function(Object)' or 'FutureOr<Null> Function(Object, StackTrace)'.
}
