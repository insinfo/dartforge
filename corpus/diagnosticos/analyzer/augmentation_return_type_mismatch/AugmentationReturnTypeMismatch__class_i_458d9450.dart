abstract class A {
  int get foo;
}

abstract class B implements A {
  get foo;
  augment String get foo;
//        ^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'String' must be the same as the introductory declaration's return type 'int'.
}
