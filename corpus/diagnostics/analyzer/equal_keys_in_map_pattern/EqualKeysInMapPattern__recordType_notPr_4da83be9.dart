void f(x) {
  if (x case {(0, const A()): 1, (0, const A()): 2}) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}
