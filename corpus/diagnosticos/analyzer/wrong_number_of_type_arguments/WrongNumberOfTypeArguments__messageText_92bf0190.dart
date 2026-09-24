void f() {
  g<int>();
// ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'g' is declared with 2 type parameters, but 1 type arguments are given.
}
void g<T, U>() {}
