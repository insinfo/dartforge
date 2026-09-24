extension type A(int it) {
  void foo() {}
}

augment extension type A {
  augment int foo();
//        ^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'int' must be the same as the introductory declaration's return type 'void'.
}
