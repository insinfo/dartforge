void g(Future Function() fun) {}

void f() {
  g(() async => {1});
}
