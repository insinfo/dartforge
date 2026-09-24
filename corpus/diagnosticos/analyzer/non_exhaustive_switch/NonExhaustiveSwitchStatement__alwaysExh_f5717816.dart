void f<T extends bool>(T x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'T' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    case true:
      break;
  }
}
