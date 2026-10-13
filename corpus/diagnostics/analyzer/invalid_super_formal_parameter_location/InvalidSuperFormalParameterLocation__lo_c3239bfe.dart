void f() {
  // ignore:unused_element
  void g(super.a) {}
//       ^^^^^
// [diag.invalidSuperFormalParameterLocation] Super parameters can only be used in non-redirecting generative constructors.
}
