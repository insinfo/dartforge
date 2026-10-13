class A {
  operator [](a) {}
  operator []=(a, b) {}
}
f(A a) {
  a[0];
  a[0] = 1;
}
