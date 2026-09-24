class A {
  void call(int p) {}
}
void defaultFunc(int p) {}
void f({void Function(int) a = defaultFunc}) {}
void g(A a) {
  f(a: a);
}
