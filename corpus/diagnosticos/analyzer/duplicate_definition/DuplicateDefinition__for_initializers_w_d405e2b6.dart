// %before-language-feature: wildcard-variables

f() {
  for (int _ = 0, _ = 0; ;) {}
//         ^
// [context 1] The first definition of this name.
//                ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
}
