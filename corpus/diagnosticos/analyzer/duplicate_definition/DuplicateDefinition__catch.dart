main() {
  try {} catch (e, e) {}
//              ^
// [context 1] The first definition of this name.
//                 ^
// [diag.duplicateDefinition][context 1] The name 'e' is already defined.
// [diag.unusedCatchStack] The stack trace variable 'e' isn't used and can be removed.
}