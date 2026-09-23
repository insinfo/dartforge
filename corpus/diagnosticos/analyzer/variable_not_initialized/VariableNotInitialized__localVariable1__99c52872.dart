void f() {
  const int v;
//          ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}
