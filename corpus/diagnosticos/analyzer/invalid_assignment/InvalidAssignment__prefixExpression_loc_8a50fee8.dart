class A {
  A operator+(_) => this;
}

f(A a) {
  ++a;
}
