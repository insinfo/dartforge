void f(
    Future<dynamic> future, Future<String> Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
