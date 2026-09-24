sealed class A {}

class B implements A {}

mixin M implements A {}

void f(A x) {
  switch (x) {
    case B _:
    case M _:
      break;
  }
}
