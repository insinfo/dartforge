void f(Future<int> future, void Function() callback) {
  future.then<Null>((_) => null, onError: (_, _) => callback());
}
