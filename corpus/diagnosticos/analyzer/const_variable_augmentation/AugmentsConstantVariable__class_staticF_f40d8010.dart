class A {
  static const int foo = 0;
  augment static abstract final int foo;
//                                  ^^^
// [diag.augmentsConstantVariable] Const variables can't be augmented.
}
