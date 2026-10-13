class A {
  const A();
}

typedef B<T, U> = A;

@B<int>()
//^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The class 'A' is declared with 2 type parameters, but 1 type arguments are given.
void f() {}
