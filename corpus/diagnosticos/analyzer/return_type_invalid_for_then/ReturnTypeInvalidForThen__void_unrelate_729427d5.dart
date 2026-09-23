void f(Future<void> future, String Function(dynamic, StackTrace) cb) {
  future.then<void>((_) => 1, onError: cb);
}
