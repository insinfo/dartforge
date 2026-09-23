void f(Future<void> future, Future<void> Function({Object a}) callback) {
  future.then<void>((_) {}, onError: callback);
//                                   ^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Future<void> Function({Object a})' can't be assigned to the parameter type 'FutureOr<void> Function(Object)' or 'FutureOr<void> Function(Object, StackTrace)'.
}
