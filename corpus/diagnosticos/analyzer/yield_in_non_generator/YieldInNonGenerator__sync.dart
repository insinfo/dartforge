f() {
  yield 0;
//^^^^^
// [diag.expectedToken] Expected to find ';'.
// [diag.undefinedIdentifier] Undefined name 'yield'.
}
