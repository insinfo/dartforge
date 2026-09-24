class A {}

extension E on A {
  int operator[](int index) => 0;
  void operator[]=(int index, int value) {}
}

f(A a) {
  E(a)[0] += 1;
}
