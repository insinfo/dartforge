class A {
  A operator +(double p) => this;
  A operator -(double p) => this;
}

void f(A a) {
  ++a;
  --a;
  a++;
  a--;
}
