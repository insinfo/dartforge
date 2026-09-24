void f(Future<int> future) {
  future.then<dynamic>((_) => 0, onError: (e, st) async {
    return;
  });
}
