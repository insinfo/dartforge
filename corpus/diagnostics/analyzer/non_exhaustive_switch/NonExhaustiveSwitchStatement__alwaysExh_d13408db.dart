sealed class A {}

class B implements A {}

mixin M implements A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'M()'.
    case B _:
      break;
  }
}
