f() {
  for (int i = 0; 3;) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
//                ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}
