void f(bool x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'bool' can never be equal to this constant of type 'int'.
}
