import 'dart:async';
void f(StreamSubscription<void> subscription) {
  subscription.onError((Object a, StackTrace? b) {});
}
