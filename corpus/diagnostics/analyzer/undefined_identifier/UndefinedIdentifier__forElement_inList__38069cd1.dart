f() {
  return [for (int x in []) null, x];
//                 ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
//                                ^
// [diag.undefinedIdentifier] Undefined name 'x'.
}
