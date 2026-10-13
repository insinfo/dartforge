sealed class A {}

class B implements A {}

enum E implements A {
  a, b
}

void f(A x) {
  switch (x) {
    case B _:
    case E.a:
    case E.b:
      break;
  }
}
