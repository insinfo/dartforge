class C({this._123}) {
//            ^^^^
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
  int? _123;
//     ^^^^
// [diag.unusedField] The value of the field '_123' isn't used.
}
