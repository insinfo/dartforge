void f() {
  [for (var x = x;;) x];
//          ^
// [context 1] The declaration of 'x' is here.
//              ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'x' can't be referenced before it is declared.
}
