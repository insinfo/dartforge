void f(Future<int> future, Future<int> Function([Object a]) callback) {
  future.catchError(callback);
}
