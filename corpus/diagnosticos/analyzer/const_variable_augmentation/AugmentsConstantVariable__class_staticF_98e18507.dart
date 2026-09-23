class A {
  static const int foo = 0;
  augment static void set foo(int _);
//                        ^^^
// [diag.augmentsConstantVariable] Const variables can't be augmented.
}
