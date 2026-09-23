void f(Future<dynamic> future, String Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
