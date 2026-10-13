class C {
  C([@Deprecated.optional() int? p]);
  C.two() : this();
//          ^^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
