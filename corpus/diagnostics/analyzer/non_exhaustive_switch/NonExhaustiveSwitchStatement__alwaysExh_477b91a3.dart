enum E {
  a, b
}

void f(E x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.a'.
    case E.a when 1 == 0:
    case E.b:
      break;
  }
}
