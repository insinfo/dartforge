void f<T>(T x) {
  if (x is bool) {
    switch (x) {
//  ^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'T & bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
      case true:
        break;
    }
  }
}
