extension type A(int it) {
  external static const int v;
//                          ^
// [diag.constNotInitialized] The constant 'v' must be initialized.
}
