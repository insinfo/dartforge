void f(C c) {
  c.g<int>();
//   ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'g' is declared with 2 type parameters, but 1 type arguments are given.
}
class C {
  void g<T, U>() {}
}
