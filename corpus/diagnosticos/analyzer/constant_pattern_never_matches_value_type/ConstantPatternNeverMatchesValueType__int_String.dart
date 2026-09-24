void f(String x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'String' can never be equal to this constant of type 'int'.
}
