void f(Future<int> future, Future<int> Function(dynamic a) callback) {
  future.catchError(callback);
}
