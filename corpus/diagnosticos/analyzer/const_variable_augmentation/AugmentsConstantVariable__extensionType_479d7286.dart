extension type A(int it) {
  static const int foo = 0;
  augment static int get foo;
//                       ^^^
// [diag.augmentsConstantVariable] Const variables can't be augmented.
}
