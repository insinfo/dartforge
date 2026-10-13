import 'dart:async';
void f(StreamSubscription<void> subscription) {
  subscription.onError((String a) {});
//                     ^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(String)' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
