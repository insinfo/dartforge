void f() {
  g<int, String>();
// ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'g' is declared with 1 type parameters, but 2 type arguments are given.
}
void g<T>() {}
