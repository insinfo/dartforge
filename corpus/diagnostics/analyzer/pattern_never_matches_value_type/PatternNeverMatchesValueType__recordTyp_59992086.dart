void f(({int a, int b}) x) {
  if (x case ({int f1, int f2,}) _) {}
//           ^^^^^^^^^^^^^^^^^^^
// [diag.patternNeverMatchesValueType] The matched value type '({int a, int b})' can never match the required type '({int f1, int f2})'.
}
