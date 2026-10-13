extension type A(Future<int> it) implements Future<int> {}

void f(A a) async {
  await a;
}
