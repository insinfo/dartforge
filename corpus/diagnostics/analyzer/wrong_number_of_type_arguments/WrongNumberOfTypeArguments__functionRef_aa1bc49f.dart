f() {
  void foo<T>() {}
  foo<int, int>;
//   ^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 1 type parameters, but 2 type arguments are given.
}
