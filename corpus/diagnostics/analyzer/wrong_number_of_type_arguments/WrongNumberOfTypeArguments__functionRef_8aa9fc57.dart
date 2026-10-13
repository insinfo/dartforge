f(C c) {
  c<int>;
// ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'call' is declared with 2 type parameters, but 1 type arguments are given.
}
class C {
  void call<T, U>() {}
}
