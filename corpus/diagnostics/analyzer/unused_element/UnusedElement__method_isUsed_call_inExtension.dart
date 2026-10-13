extension<T> on T {
  void call() {}
}

void f() {
  (<T>(T t) => t())(7);
}
