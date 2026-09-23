void f(Stream<void> stream, Future<int> Function({Object a}) callback) {
  stream.handleError(callback);
//                   ^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Future<int> Function({Object a})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
