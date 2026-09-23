class C {
  C();
  factory C.two([int? p]) = D;
}

class D extends C {
  D([@Deprecated.optional() int? p]);
}
