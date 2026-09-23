void f(Future<int> future, int Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
