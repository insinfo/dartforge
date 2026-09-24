extension type A(Future<int> it) implements Future<int> {}

void f<T extends A>(T a) async {
  await a;
}
