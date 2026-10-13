void f(String x) {
  if (x case int _) {}
//           ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'String' can never match the required type 'int'.
}
