class C {
  C();
  factory C.two() = D.two;
}

class D extends C {
  D();
  factory D.two() = E;
//        ^^^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}

class E extends D {
  E({@Deprecated.optional() int? p});
}
