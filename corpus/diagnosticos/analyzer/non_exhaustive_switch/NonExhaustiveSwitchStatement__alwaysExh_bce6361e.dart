sealed class A {}
class B extends A {}

void f(A x) {
  switch (x) {
    case Unresolved():
//       ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
      break;
  }
}
