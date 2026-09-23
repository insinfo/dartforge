import 'dart:async' as a;
//     ^^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:async'.
f() {
  a.Future? x = null;
//^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'a' can't be used here because it's shadowed by a local declaration.
// [diag.referencedBeforeDeclaration][context 1] Local variable 'a' can't be referenced before it is declared.
  int a = 0;
//    ^
// [context 1] The declaration of 'a' is here.
  return [x, a];
}
