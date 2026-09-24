import 'dart:async';
void f(Future<int> future, FutureOr<int> Function(dynamic, StackTrace) cb) {
  future.then<int>((_) => 1, onError: cb);
}
