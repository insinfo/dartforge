void f(x) {
  if (x case {(a: const A()): 1, (a: const A()): 2}) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}
