// %before-language-feature: private-named-parameters
class C {
  int? _x;
//     ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  C({this._x});
//        ^^
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
}
