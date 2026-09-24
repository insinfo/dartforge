void f(Future<int> future, void Function(dynamic, StackTrace) cb) {
  future.then<int?>((_) => 1, onError: cb);
}
