class A {
  int operator[](int index) => 0;

  operator[]=(int index, int value) {}
}

extension E on A {
  void bar() {
    this[0];

    this[0] = 0;
  }
}
