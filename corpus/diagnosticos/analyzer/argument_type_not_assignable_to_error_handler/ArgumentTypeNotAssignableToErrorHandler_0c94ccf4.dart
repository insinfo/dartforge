void f(Future<void> future, Function callback) {
  future.then((_) {}, onError: callback);
}
