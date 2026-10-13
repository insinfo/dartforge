enum E<T, U> {
  v<int>()
// ^^^^^
// [diag.wrongNumberOfTypeArgumentsEnum] The enum is declared with 2 type parameters, but 1 type arguments were given.
}
