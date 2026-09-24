void f(Future<int> future, Future<int> Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
