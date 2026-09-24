// %before-language-feature: wildcard-variables

class A {
  int _;
//    ^
// [diag.unusedField] The value of the field '_' isn't used.
  A({required this._, required this._});
//                 ^
// [context 1] The first definition of this name.
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
//                                  ^
// [diag.duplicateFieldFormalParameter][context 1] The field '_' can't be initialized by multiple parameters in the same constructor.
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
}
