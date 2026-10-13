void f(Future<int> future) {
  future.then((_) => 0, onError: (e, st) => 0);
}
