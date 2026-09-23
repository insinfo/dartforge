import 'dart:async';
void f(StreamSubscription<void> subscription) {
  subscription.onError((Object a, {StackTrace? b}) {});
//                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function(Object, {StackTrace? b})' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
