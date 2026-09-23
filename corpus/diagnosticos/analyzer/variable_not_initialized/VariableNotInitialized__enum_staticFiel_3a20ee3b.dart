enum A {
  e;
  static const int v;
//                 ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
}
