class C {
  final String? _foo;
//              ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
  C({String? _foo, required this._foo}) {}
//           ^^^^
// [context 1] The first definition of this name.
// [diag.privateNamedNonFieldParameter] Named parameters that don't refer to instance variables can't start with underscore.
//                               ^^^^
// [diag.duplicateDefinition][context 1] The name '_foo' is already defined.
}
