void f(Future<void> future) {
  future.catchError((a) {});
}
