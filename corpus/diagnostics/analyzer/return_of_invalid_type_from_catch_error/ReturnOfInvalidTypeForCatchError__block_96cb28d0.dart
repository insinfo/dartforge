void f(Future<dynamic> future) {
  future.catchError((e, st) {
    return;
  });
}
