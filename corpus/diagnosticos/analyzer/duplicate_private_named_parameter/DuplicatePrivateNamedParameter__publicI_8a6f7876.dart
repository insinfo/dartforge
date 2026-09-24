class C {
  final String? foo;
  final String? _foo;
//              ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
  C({required this.foo, required this._foo}) {}
//                 ^^^
// [context 1] The first definition of this name.
//                                    ^^^^
// [diag.privateNamedParameterDuplicatePublicName][context 1] The corresponding public name 'foo' is already the name of another parameter.
}
