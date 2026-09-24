enum E { one, two, three }

void f(E e) {
  switch (e) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.three'.
    case E.one:
    case E.two:
      break;
  }
}
