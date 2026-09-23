void f(Future<int> future, String Function(dynamic, StackTrace) cb) {
  future.then<dynamic>((_) => 1, onError: cb);
}
