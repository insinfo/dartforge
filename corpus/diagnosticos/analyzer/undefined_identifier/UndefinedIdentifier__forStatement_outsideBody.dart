f() {
  for (int x in []) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  x;
//^
// [diag.undefinedIdentifier] Undefined name 'x'.
}
