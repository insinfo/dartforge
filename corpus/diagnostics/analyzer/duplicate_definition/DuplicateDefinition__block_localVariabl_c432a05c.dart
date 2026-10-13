// %before-language-feature: wildcard-variables

void f() {
  var _ = 0;
//    ^
// [context 1] The first definition of this name.
  var _ = 1;
//    ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
}
