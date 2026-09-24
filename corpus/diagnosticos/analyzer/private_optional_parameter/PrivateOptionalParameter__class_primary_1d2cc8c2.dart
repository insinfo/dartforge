class C({this._}) {
//            ^
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
  int? _;
//     ^
// [diag.unusedField] The value of the field '_' isn't used.
}
