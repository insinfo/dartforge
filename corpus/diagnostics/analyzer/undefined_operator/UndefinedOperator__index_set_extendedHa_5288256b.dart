class A {}

extension E on A {
  void operator[]=(int index, int value) {}
}

f(A a) {
  a[0] = 1;
}
