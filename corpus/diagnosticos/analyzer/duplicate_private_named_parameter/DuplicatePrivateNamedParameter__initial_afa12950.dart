class C {
  final String? _foo;
//              ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
  C({required this._foo, String? foo}) {}
//                 ^^^^
// [diag.privateNamedParameterDuplicatePublicName][context 1] The corresponding public name 'foo' is already the name of another parameter.
//                               ^^^
// [context 1] The first definition of this name.
}
