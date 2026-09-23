f() {
  int i;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
  for (i = 0; 3;) {}
//            ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}