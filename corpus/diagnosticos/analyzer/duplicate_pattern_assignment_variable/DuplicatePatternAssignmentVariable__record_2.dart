void f() {
  int a;
  (a, a) = (1, 2);
// ^
// [context 1] The first assigned variable pattern.
//    ^
// [diag.duplicatePatternAssignmentVariable][context 1] The variable 'a' is already assigned in this pattern.
  a;
}
