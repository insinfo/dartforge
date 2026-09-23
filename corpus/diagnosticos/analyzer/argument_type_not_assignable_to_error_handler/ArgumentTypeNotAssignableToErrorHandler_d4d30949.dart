import 'dart:async';
void f(StreamSubscription<void> subscription) {
  subscription.onError(({Object a = 1}) {});
//                     ^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function({Object a})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
