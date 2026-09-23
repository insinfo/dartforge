import 'dart:async' as a;
//     ^^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:async'.
f() {
  int a = 0;
  a.Future? x = null;
//^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'a' can't be used here because it's shadowed by a local declaration.
  return [x, a];
}
