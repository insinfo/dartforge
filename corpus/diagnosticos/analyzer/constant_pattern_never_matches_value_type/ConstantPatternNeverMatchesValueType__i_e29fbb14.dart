extension type E(bool it) {}

void f(E x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'bool' can never be equal to this constant of type 'int'.
}
