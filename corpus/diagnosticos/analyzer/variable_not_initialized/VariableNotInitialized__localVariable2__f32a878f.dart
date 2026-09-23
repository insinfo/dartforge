void f() {
  const int v1, v2;
//          ^^
// [diag.constNotInitialized] The constant 'v1' must be initialized.
// [diag.unusedLocalVariable] The value of the local variable 'v1' isn't used.
//              ^^
// [diag.constNotInitialized] The constant 'v2' must be initialized.
// [diag.unusedLocalVariable] The value of the local variable 'v2' isn't used.
}
