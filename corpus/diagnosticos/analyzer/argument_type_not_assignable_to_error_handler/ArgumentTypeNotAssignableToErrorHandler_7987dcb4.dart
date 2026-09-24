void f(Future<int> future, Future<int> Function(Object a, dynamic b) callback) {
  future.catchError(callback);
}
