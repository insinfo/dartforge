var v = 0;

void f(int a) {
  switch (a) {
    case 0:
      v;
//    ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'v' can't be referenced before it is declared.
      var v = 1;
//        ^
// [context 1] The declaration of 'v' is here.
  }
}
