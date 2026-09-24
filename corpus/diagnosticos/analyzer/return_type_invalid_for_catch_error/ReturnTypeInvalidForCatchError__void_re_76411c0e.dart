void f(Future<void> future, Future<String> Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
