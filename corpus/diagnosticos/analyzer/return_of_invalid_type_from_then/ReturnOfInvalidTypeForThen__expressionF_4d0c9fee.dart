void f(Future<int> future) {
  future.then<Null>((_) => null, onError: (e, st) => null);
}
