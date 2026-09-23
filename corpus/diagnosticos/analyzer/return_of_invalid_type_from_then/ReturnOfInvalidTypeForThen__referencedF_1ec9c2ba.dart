void f(Future<int> future, Future<Object?> Function(dynamic, StackTrace) callback) {
  future.then<void>((_) {}, onError: callback);
}
