class A {
  A(super.a) : this._();
//  ^^^^^
// [diag.invalidSuperFormalParameterLocation] Super parameters can only be used in non-redirecting generative constructors.
  A._();
}
