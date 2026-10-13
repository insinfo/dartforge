void g(void Function() fun) {}

void f() {
  g(() => {});
}
