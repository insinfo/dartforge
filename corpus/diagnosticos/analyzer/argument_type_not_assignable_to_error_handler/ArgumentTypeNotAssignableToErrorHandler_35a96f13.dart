void f(Future<void> future) {
  future.catchError((Object a, String b) {});
//                  ^^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(Object, String)' can't be assigned to the parameter type 'FutureOr<void> Function(Object)' or 'FutureOr<void> Function(Object, StackTrace)'.
}
