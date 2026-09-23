extension E on int {
  void foo(super.a) {}
//         ^^^^^
// [diag.invalidSuperFormalParameterLocation] Super parameters can only be used in non-redirecting generative constructors.
}
