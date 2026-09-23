void f(String x) {
  if (x case int a) {}
//           ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'String' can never match the required type 'int'.
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
