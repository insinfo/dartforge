class A {
  int? _p;
//     ^^
// [diag.unusedField] The value of the field '_p' isn't used.
  A({this._p = 0});
}
