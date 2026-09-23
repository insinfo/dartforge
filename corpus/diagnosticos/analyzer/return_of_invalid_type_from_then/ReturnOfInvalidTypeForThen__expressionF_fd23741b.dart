void f(Future<int> future) {
  future.then<void>((_) => 0, onError: (e, st) => 0);
}
