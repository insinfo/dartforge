void g(Function() fun) {}

void f() {
  g(() => {1});
}
