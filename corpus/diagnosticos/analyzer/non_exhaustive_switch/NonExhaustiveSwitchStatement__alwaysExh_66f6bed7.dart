void f(bool x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'true'.
    case int _:
//       ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'bool' can never match the required type 'int'.
      break;
  }
}
