void f(Future<int> future, void Function(dynamic, StackTrace) callback) {
  future.then<Null>((_) => null, onError: callback);
}
