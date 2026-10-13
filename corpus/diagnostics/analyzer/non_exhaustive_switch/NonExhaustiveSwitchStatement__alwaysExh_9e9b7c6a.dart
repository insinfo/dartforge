sealed class A {}
class B extends A {}

void f(A x) {
  switch (x) {
    case unresolved:
//       ^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'unresolved'.
      break;
  }
}
