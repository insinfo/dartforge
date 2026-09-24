extension E on int {
  void foo() {}
}

void f() {
  E<int>(0).foo();
// ^^^^^
// [diag.wrongNumberOfTypeArgumentsExtension] The extension 'E' is declared with 0 type parameters, but 1 type arguments were given.
}
