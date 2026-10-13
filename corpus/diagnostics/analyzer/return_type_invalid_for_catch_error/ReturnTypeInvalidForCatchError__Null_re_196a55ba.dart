void f(Future<Null> future, void Function(dynamic, StackTrace) cb) {
  future.catchError(cb);
}
