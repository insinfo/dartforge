void f(bool x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    case true:
      break;
  }
}
