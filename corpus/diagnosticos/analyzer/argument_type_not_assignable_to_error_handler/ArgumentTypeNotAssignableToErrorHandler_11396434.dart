void f(Future<int> future, Future<int> Function(String) callback) {
  future.catchError(callback);
//                  ^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Future<int> Function(String)' can't be assigned to the parameter type 'FutureOr<int> Function(Object)' or 'FutureOr<int> Function(Object, StackTrace)'.
}
