void f(Future<int> future) {
  future.catchError((e, st) {
    return 7;
  });
}
