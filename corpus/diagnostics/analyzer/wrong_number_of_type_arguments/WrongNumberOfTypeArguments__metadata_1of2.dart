class A<T, U> {
  const A();
}

@A<int>()
//^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The class 'A' is declared with 2 type parameters, but 1 type arguments are given.
void f() {}
