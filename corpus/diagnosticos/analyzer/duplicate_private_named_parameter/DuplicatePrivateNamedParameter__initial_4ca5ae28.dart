class C {
  final String? _foo;
//              ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
  C({required this._foo, required this._foo}) {}
//                 ^^^^
// [context 1] The first definition of this name.
//                                     ^^^^
// [diag.duplicateFieldFormalParameter][context 1] The field '_foo' can't be initialized by multiple parameters in the same constructor.
}
