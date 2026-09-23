main() {
  var v = v;
//    ^
// [context 1] The declaration of 'v' is here.
//        ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'v' can't be referenced before it is declared.
}
