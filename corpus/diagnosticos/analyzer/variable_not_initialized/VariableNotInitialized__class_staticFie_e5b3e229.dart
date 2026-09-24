class A {
  static const int v1, v2;
//                 ^^
// [diag.constNotInitialized] The constant 'v1' must be initialized.
//                     ^^
// [diag.constNotInitialized] The constant 'v2' must be initialized.
}
