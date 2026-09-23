class A {
  const A();
}

@A<int>()
//^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The class 'A' is declared with 0 type parameters, but 1 type arguments are given.
void f() {}
