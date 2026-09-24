void f(Future<void> future) {
  future.catchError((a, b, c) {});
//                  ^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(dynamic, dynamic, dynamic)' can't be assigned to the parameter type 'FutureOr<void> Function(Object)' or 'FutureOr<void> Function(Object, StackTrace)'.
}
