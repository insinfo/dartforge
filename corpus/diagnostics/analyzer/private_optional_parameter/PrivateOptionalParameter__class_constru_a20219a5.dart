class C {
  int? _for;
//     ^^^^
// [diag.unusedField] The value of the field '_for' isn't used.
  C({this._for}) {}
//        ^^^^
// [diag.privateNamedParameterWithoutPublicName] A private named parameter must be a public identifier after removing the leading underscore.
}
