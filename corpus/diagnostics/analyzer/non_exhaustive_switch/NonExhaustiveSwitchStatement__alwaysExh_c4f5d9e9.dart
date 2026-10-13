sealed class A {}
class B extends A {}
extension type EA(A it) implements A {}

void f(A x) {
  switch (x) {
    case B():
      break;
  }
}
