class A {
  A(int a);
}

void f() {
  const A(0);
//^^^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
}
