void f(Future<int> future) {
  future.then<void>((_) {}, onError: (e, st) {
    return Future<Object?>.error(0.5);
  });
}
