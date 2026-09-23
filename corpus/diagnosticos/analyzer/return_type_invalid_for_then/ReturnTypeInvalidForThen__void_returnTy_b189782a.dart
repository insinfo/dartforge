void f(Future<void> future, Future<String> Function(dynamic, StackTrace) cb) {
  future.then<void>((_) => 1, onError: cb);
}
