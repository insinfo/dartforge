class A {
  void call(int p) {}
}
void f(void Function(int) a) {}
void g(A a) {
  f(a);
}
