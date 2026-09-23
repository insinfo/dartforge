void f({@Deprecated.optional() int? p}) {}

void g() {
  f();
//^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
