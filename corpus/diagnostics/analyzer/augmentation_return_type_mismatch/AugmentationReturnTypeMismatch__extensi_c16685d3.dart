extension type A(int it) {
  int get foo => 0;
}

augment extension type A {
  augment String get foo;
//        ^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'String' must be the same as the introductory declaration's return type 'int'.
}
