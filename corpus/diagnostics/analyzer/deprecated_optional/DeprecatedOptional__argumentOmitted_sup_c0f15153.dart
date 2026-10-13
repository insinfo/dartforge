class C {
  C([@Deprecated.optional() int? p]);
}

class D extends C {
  D() : super();
//      ^^^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
