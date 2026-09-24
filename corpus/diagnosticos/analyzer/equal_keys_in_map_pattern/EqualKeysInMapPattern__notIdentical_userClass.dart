void f(x) {
  if (x case {const A(0): 1, const A(2): 3}) {}
}

class A {
  final int field;
  const A(this.field);
  bool operator ==(other) => false;
}
