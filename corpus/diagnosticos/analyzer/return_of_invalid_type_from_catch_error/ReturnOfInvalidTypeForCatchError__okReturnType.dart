void f(Future<int> future) {
  future.catchError((e, st) => 0);
}
