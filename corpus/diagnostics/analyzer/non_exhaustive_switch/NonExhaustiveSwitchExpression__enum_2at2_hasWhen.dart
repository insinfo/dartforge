enum E {
  a, b
}

Object f(E x) {
  return switch (x) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.a'.
    E.a when 1 == 0 => 0,
    E.b => 1,
  };
}
