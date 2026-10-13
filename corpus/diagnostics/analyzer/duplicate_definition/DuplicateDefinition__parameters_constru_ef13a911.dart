class A {
  int? _;
//     ^
// [diag.unusedField] The value of the field '_' isn't used.
  A(int _, this._);
}
