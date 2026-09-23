class C {
  C.named([@Deprecated.optional() int? p]);
}

class D extends C {
  D() : super.named();
//            ^^^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
