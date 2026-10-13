void f(Future<void> future) {
  future.then((_) {}, onError: (Object a, StackTrace? b) {});
}
