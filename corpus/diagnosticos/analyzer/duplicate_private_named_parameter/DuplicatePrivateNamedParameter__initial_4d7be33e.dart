class C {
  final String? _foo;
//              ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
  C({required this._foo, String? _foo}) {}
//                 ^^^^
// [context 1] The first definition of this name.
//                               ^^^^
// [diag.duplicateDefinition][context 1] The name '_foo' is already defined.
// [diag.privateNamedNonFieldParameter] Named parameters that don't refer to instance variables can't start with underscore.
}
