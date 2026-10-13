// %before-language-feature: wildcard-variables

class A {
  int _;
//    ^
// [diag.unusedField] The value of the field '_' isn't used.
  A({this._ = 0, this._ = 1});
//        ^
// [context 1] The first definition of this name.
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
//                    ^
// [diag.duplicateFieldFormalParameter][context 1] The field '_' can't be initialized by multiple parameters in the same constructor.
// [diag.experimentNotEnabled] This requires the 'private-named-parameters' language feature to be enabled.
}
