extension type A(Future<int> it) implements Future<int> {}

void f<T>(T a) async {
  if (T is A) {
    await a;
  }
}
