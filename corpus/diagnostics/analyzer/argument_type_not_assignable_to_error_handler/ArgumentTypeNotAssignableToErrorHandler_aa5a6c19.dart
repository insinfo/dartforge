void f(Future<void> future) {
  future.catchError((String a) {});
//                  ^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(String)' can't be assigned to the parameter type 'FutureOr<void> Function(Object)' or 'FutureOr<void> Function(Object, StackTrace)'.
}
