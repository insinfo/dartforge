extension E on int {
  int get foo => 0;
}

augment extension E {
  augment String get foo;
//        ^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'String' must be the same as the introductory declaration's return type 'int'.
}
