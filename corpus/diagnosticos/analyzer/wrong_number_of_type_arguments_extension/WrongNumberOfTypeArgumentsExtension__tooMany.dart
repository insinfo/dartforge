extension E<T> on int {
  void foo() {}
}

void f() {
  E<bool, int>(0).foo();
// ^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsExtension] The extension 'E' is declared with 1 type parameters, but 2 type arguments were given.
}
