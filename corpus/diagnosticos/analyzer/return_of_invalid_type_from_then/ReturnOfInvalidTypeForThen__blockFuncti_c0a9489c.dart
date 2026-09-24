void f(Future<void> future) {
  future.then<void>((_) => 0, onError: (e, st) async {
    return;
  });
}
