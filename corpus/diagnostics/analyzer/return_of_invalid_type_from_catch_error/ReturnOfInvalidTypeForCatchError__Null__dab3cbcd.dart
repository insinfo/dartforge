void f(Future<Null> future) {
  future.catchError((e, st) => null);
}
