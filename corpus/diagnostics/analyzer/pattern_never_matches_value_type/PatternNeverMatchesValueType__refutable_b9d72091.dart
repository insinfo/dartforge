void f(String x) {
  if (x case _ as int) {}
//                ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'String' can never match the required type 'int'.
}
