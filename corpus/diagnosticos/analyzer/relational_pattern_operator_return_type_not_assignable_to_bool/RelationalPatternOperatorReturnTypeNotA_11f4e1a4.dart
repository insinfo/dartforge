class A {
  dynamic operator >(_) => 42;
}

void f(A x) {
  if (x case > 0) {}
}
