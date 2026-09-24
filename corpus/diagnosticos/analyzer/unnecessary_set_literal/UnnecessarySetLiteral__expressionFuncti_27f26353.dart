void g(void Function() fun) {}

void f() {
  g(() => {1: 2});
}
