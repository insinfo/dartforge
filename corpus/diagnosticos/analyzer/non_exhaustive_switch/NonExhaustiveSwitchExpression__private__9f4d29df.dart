enum _E { a, b }
//        ^
// [diag.unusedField] The value of the field 'a' isn't used.
//           ^
// [diag.unusedField] The value of the field 'b' isn't used.

Object f(_E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type '_E' isn't exhaustively matched by the switch cases since it doesn't match the pattern '_E.a'.
  };
}
