class C {
  C();
  factory C.two() = D;
//        ^^^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}

class D extends C {
  D([@Deprecated.optional() int? p]);
}
