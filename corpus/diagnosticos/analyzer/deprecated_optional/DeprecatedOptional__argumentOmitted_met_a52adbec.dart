class C {
  void m({@Deprecated.optional() int? p}) {}
}

void f(C c) {
  c..m();
//   ^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
