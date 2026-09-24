void f(Future<void> future, String Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
