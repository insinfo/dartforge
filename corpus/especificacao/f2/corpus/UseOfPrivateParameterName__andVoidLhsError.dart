class C {
  int? _x;
//     ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  C({this._x});
}
void f() {
  C(_x: 123);
//  ^^
// [diag.useOfPrivateParameterName] The named parameter '_x' should use the corresponding public name 'x' at the callsite.
}
