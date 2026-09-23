class C {
  C.named([@Deprecated.optional() int? p]);
}

class D extends C {
  D() : super.named(7);
}
