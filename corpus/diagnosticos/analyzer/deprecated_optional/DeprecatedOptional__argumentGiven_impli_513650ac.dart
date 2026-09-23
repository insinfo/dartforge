class C {
  C([@Deprecated.optional() int? p]);
}

class D extends C {
  D(super.p);
}
