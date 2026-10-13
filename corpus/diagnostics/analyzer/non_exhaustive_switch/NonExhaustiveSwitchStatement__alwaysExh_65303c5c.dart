sealed class A {}
class B extends A {}
class C extends A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'C()'.
    case B():
      break;
  }
}
