class A {
  static final int v1 = 0, v2, v3 = 0;
//                         ^^
// [diag.finalNotInitialized] The final variable 'v2' must be initialized.
  A();
}
