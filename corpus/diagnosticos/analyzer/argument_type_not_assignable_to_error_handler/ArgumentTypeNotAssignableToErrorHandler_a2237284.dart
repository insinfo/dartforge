void f(Stream<void> stream) {
  stream.handleError(() {});
//                   ^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function()' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
