void g(Future<Object> Function() fun) {}

void f() {
  g(() async => {1});
}
