class C {
  C({@Deprecated.optional() int? p});
}

C f() {
  return .new();
//        ^^^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
