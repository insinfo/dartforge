void f(Future<int> future) {
  future.catchError((e, st) async => 0);
}
