extension E on int {
  void foo() {}
}

augment extension E {
  augment int foo();
//        ^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'int' must be the same as the introductory declaration's return type 'void'.
}
