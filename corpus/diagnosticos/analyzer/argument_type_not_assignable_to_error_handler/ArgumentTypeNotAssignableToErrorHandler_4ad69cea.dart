void f(Future<int> future, Future<int> Function({Object a}) callback) {
  future.catchError(callback);
//                  ^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Future<int> Function({Object a})' can't be assigned to the parameter type 'FutureOr<int> Function(Object)' or 'FutureOr<int> Function(Object, StackTrace)'.
}
