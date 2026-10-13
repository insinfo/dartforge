sealed class A {}

class B implements A {}

enum E implements A {
  a, b
}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.b'.
    case B _:
    case E.a:
      break;
  }
}
