enum E { a, b, _c }
//             ^^
// [diag.unusedField] The value of the field '_c' isn't used.
Object f(E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E._c'.
    E.a => 0,
    E.b => 1,
  };
}
