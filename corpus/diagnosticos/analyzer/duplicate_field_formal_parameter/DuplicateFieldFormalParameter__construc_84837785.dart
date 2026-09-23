// %before-language-feature: wildcard-variables

class A {
  final _;
//      ^
// [diag.unusedField] The value of the field '_' isn't used.
  A([this._ = 1, this._ = 2]) {}
//        ^
// [context 1] The first definition of this name.
//                    ^
// [diag.duplicateFieldFormalParameter][context 1] The field '_' can't be initialized by multiple parameters in the same constructor.
}
