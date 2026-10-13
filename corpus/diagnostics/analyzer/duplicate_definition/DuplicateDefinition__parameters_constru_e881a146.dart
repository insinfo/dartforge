class A {
  int? _;
//     ^
// [diag.unusedField] The value of the field '_' isn't used.
  A(this._);
}
class B extends A {
  B(super._, super._);
//                 ^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
}
