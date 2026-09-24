sealed class A {}
class B extends A {}
class C extends A {}

void f(A x) {
  switch (x) {
    case B():
      break;
    case C():
      break;
  }
}
