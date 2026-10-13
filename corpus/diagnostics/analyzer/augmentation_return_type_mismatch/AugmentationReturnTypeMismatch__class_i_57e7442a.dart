abstract class A {
  int foo();
}

abstract class B implements A {
  foo();
  augment String foo();
//        ^^^^^^
// [diag.augmentationReturnTypeMismatch] The augmentation's return type 'String' must be the same as the introductory declaration's return type 'int'.
}
