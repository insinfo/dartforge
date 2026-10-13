void f(Future<void> future) {
  future.catchError((e) {
    return;
  });
}
