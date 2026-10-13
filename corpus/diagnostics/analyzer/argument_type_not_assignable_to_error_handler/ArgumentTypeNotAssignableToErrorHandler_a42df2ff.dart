void f(Future<void> future) {
  future.catchError((Object a, dynamic b) {});
}
