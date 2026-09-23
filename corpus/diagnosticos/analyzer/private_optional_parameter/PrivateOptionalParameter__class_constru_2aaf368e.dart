// %before-language-feature: private-named-parameters
class C {
  int? _123;
//     ^^^^
// [diag.unusedField] The value of the field '_123' isn't used.
  C({this._123}) {}
//        ^^^^
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
}
