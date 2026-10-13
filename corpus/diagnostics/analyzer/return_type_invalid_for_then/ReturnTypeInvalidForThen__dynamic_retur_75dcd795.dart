void f(
    Future<int> future, Future<String> Function(dynamic, StackTrace) cb) {
  future.then<dynamic>((_) => 1, onError: cb);
}
