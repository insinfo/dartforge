void f(Future<int> future, Future<int> Function(Object, StackTrace) callback) {
  future.catchError(callback);
}
