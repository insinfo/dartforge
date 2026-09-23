class A {
  void call(int p) {}
}
void f({required void Function(int) a}) {}
void g(A a) {
  f(a: a);
}
