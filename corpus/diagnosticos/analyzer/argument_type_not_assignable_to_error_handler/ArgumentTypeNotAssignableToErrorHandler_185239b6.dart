void f(Future<void> future, void Function(dynamic a) callback) {
  future.then((_) {}, onError: callback);
}
