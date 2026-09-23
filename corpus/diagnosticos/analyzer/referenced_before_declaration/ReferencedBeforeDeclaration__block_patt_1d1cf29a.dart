var v = 0;
void f() {
  v;
//^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'v' can't be referenced before it is declared.
  var [v] = [0];
//     ^
// [context 1] The declaration of 'v' is here.
}
