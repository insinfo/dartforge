Object f(bool x) {
  return switch (x) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    true => 0,
  };
}
