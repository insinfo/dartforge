extension E<S, T> on int {
  void foo() {}
}

void f() {
  E<bool>(0).foo();
// ^^^^^^
// [diag.wrongNumberOfTypeArgumentsExtension] The extension 'E' is declared with 2 type parameters, but 1 type arguments were given.
}
