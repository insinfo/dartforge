// %before-language-feature: patterns
var v = 0;

void f(int a) {
  switch (a) {
    default:
      v;
//    ^
// [diag.referencedBeforeDeclaration][context 1] Local variable 'v' can't be referenced before it is declared.
      void v() {}
//         ^
// [context 1] The declaration of 'v' is here.
  }
}
