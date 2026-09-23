class C {
  void m({@Deprecated.optional() int? p}) {}
}

class D extends C {
  @override
  void m({int? p}) {}
}

void g(D d) {
  d.m();
}
