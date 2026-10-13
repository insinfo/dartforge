class C {
  C([@Deprecated.optional() int? p]);
}

void f() {
  C();
//^
// [diag.deprecatedOptional] Omitting an argument for the 'p' parameter is deprecated.
}
