import 'dart:async';
void f(
    StreamSubscription<void> subscription, void Function(dynamic a) callback) {
  subscription.onError(callback);
}
