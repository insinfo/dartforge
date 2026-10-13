// %before-language-feature: wildcard-variables

f() {
  try {} catch (_, _) {}
//              ^
// [context 1] The first definition of this name.
//                 ^
// [diag.duplicateDefinition][context 1] The name '_' is already defined.
// [diag.unusedCatchStack] The stack trace variable '_' isn't used and can be removed.
}
