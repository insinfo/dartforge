class A {
  factory A(int a) => throw 0;
}

void f() {
  const A(0);
//^^^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
}
