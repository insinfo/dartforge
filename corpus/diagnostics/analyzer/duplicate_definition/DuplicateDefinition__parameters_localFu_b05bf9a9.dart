// %before-language-feature: wildcard-variables

f() {
  g(int _, double _) {};
//^
// [diag.unusedElement] The declaration 'g' isn't referenced.
//      ^
// [context 1] The first definition of this name.
//                ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
}
