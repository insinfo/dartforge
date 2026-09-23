class A {
  int get foo => 0;
}

void f(A? a) {
  a?.foo.abs();
}
