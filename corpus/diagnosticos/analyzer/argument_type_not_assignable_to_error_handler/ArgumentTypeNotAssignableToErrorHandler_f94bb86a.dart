import 'dart:async';
void f(StreamSubscription<void> subscription) {
  subscription.onError(() {});
//                     ^^^^^
// [diag.argumentTypeNotAssignableToErrorHandler] The argument type 'Null Function()' can't be assigned to the parameter type 'void Function(Object)' or 'void Function(Object, StackTrace)'.
}
