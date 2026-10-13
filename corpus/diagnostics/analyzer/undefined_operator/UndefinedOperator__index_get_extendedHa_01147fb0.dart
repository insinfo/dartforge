class A {
  void operator[]=(int index, int value) {}
}

extension E on A {
  int operator[](int index) => 0;
}

f(A a) {
  a[0];
// ^^^
// [diag.undefinedOperator] The operator '[]' isn't defined for the type 'A'.
}
