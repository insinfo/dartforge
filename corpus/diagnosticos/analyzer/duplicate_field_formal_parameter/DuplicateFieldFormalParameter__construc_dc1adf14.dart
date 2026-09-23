class A {
  int _;
//    ^
// [diag.unusedField] The value of the field '_' isn't used.
  A({this._ = 0, this._ = 1});
//        ^
// [context 1] The first definition of this name.
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
//                    ^
// [diag.duplicateFieldFormalParameter][context 1] The field '_' can't be initialized by multiple parameters in the same constructor.
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
}
