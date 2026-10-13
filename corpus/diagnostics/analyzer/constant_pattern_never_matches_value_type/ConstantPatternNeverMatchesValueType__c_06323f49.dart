void f(B x) {
  if (x case const A()) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}

class B extends A {
  const B();
}
