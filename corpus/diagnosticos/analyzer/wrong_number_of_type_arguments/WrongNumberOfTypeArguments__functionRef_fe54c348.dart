f(C c) {
  c<int, String>;
// ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'call' is declared with 1 type parameters, but 2 type arguments are given.
}
class C {
  void call<T>() {}
}
