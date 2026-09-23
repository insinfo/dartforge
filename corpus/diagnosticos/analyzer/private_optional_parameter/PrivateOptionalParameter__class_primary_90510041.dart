class C({this.__extraPrivate}) {
//            ^^^^^^^^^^^^^^
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
  int? __extraPrivate;
//     ^^^^^^^^^^^^^^
// [diag.unusedField] The value of the field '__extraPrivate' isn't used.
}
