void f(Future<int> future) {
  future.catchError((e, st) {
    double g() => 0.5;
    if (g() == 0.5) return 0;
    return 1;
  });
}
