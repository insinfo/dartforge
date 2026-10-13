f() {
  for (int i = 0, i = 0; i < 5;) {}
//         ^
// [context 1] The first definition of this name.
//                ^
// [diag.duplicateDefinition][context 1] The name 'i' is already defined.
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
}
