void f(Future<void> future) {
  future.catchError((Object a, StackTrace? b) {});
}
