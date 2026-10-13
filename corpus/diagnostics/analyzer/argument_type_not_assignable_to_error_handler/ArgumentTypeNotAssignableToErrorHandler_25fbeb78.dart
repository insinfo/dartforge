import 'dart:async';
void f(
    StreamSubscription<void> subscription,
    Future<int> Function({Object a}) callback) {
  subscription.onError(callback);
//                     ^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Future<int> Function({Object a})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
