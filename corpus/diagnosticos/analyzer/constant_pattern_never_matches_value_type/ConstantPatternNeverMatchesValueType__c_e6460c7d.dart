void f(A x) {
  if (x case const B()) {}
}

class A {
  const A();
}

class B extends A {
  const B();
}
