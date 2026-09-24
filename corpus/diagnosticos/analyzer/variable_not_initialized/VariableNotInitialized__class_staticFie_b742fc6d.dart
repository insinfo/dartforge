class A {
  static int v1 = 0, v2, v3 = 0;
//                   ^^
// [diag.notInitializedNonNullableVariable] The non-nullable variable 'v2' must be initialized.
  A();
}
