// %before-language-feature: wildcard-variables

void f() {
  void _() {}
//     ^
// [context 1] The first definition of this name.
// [context 2] The first definition of this name.
// [diag.unusedElement] The declaration '_' isn't referenced.
  int _(int _) => 42;
//    ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
// [diag.unusedElement] The declaration '_' isn't referenced.
  String _(int _) => "42";
//       ^
// [diag.duplicateDefinition][context 2] The name '_' is already defined.
// [diag.unusedElement] The declaration '_' isn't referenced.
}
