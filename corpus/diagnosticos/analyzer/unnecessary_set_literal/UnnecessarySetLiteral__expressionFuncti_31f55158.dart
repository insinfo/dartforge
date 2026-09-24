void g(Object Function() fun) {}

void f() {
  g(() => {1});
}
