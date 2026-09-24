const int foo = 0;
augment void set foo(int _);
//               ^^^
// [diag.augmentsConstantVariable] Const variables can't be augmented.
