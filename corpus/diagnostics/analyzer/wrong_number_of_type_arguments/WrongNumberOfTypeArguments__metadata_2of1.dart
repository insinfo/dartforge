class A<T> {
  const A();
}

@A<int, String>()
//^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The class 'A' is declared with 1 type parameters, but 2 type arguments are given.
void f() {}
