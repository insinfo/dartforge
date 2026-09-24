void f({@Deprecated.optional() int? p}) {}

void g() {
  f(p: 1);
}
