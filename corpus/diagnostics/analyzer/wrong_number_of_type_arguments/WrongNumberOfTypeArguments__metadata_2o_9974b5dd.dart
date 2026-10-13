class A {
  const A();
}

typedef B<T> = A;

@B<int, String>()
//^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The class 'A' is declared with 1 type parameters, but 2 type arguments are given.
void f() {}
