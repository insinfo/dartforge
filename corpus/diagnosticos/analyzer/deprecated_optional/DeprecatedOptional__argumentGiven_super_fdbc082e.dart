class C {
  C([@Deprecated.optional() int? p]);
}

class D(super.p) extends C {
  this : super();
}
