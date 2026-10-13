class A {
  const A();
}

typedef X = A;

void f() {
  const X();
}
