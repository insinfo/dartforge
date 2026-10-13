class C {
  C.one([@Deprecated.optional() int? p]);
  C.two() : this.one();
//               ^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
