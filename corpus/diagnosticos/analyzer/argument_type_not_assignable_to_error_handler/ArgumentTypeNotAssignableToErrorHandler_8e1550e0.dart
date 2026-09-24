void f(Future<void> future) {
  future.catchError(() {});
//                  ^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function()' can't be assigned to the parameter type 'FutureOr<void> Function(Object)' or 'FutureOr<void> Function(Object, StackTrace)'.
}
