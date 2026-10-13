class A {
  int operator[](int index) => 0;
}

extension E on A {
  void operator[]=(int index, int value) {}
}

f(A a) {
  a[0] = 1;
// ^^^
// [diag.undefinedOperator] The operator '[]=' isn't defined for the type 'A'.
}
