class A {
  void m<T>() {}
}

void f(A a) {
  a.m<int, int>();
}
