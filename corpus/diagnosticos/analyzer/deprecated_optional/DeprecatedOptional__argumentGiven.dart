void f([@Deprecated.optional() int? p]) {}

void g() {
  f(1);
}
