void f(Future<int> future, int Function(dynamic, StackTrace) cb) {
  future.then<int>((_) => 1, onError: cb);
}
