class C {
  C({@Deprecated.optional() int? p});
}

class D extends C {
  D();
//^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
