class A {
  factory A(super.a) {
//          ^^^^^
// [diag.invalidSuperFormalParameterLocation] Super parameters can only be used in non-redirecting generative constructors.
    return A._();
  }
  A._();
}
