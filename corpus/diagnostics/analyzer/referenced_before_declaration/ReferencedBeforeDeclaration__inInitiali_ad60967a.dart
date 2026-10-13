void f() {
  var a = ++a;
//    ^
// [context 1] The declaration of 'a' is here.
//          ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'a' can't be referenced before it is declared.
  var b = --b;
//    ^
// [context 2] The declaration of 'b' is here.
//          ^
// [diag.referencedBeforeDeclaration][context 2] Local variable 'b' can't be referenced before it is declared.
  var c = c++;
//    ^
// [context 3] The declaration of 'c' is here.
//        ^
// [diag.referencedBeforeDeclaration][context 3] Local variable 'c' can't be referenced before it is declared.
  var d = d--;
//    ^
// [context 4] The declaration of 'd' is here.
//        ^
// [diag.referencedBeforeDeclaration][context 4] Local variable 'd' can't be referenced before it is declared.
}
