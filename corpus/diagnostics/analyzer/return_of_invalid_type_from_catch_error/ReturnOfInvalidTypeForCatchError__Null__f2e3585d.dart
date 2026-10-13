void f(Future<Null> future, void Function() g) {
  future.catchError((e, st) => g());
}
