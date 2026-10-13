class C {
  static C m({@Deprecated.optional() int? p}) => C();
}

C f() {
  return .m();
//        ^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
