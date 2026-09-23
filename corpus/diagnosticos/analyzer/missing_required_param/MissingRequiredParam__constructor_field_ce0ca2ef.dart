class A {
  A({required this.})
//   ^^^^^^^^^^^^^^
// [diag.initializingFormalForNonExistentField] '' isn't a field in the enclosing class.
//                 ^
// [diag.missingIdentifier] Expected an identifier.
}
// [diag.missingFunctionBody][column 1][length 1] A function body must be provided.

void f() {
  A();
}
