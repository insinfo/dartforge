void f(Future<void> future) {
  future.catchError((e, st) => 0);
}
