sealed class A {}
class B extends A {}
class C extends A {}
extension type EA(A it) implements A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'C()'.
    case B():
      break;
  }
}
