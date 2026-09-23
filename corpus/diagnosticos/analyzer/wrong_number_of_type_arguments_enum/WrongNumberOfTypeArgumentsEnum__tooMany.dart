enum E<T> {
  v<int, int>()
// ^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsEnum] The enum is declared with 1 type parameters, but 2 type arguments were given.
}
