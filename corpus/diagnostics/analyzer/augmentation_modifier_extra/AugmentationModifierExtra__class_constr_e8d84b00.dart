class A {
  A();
  augment const A();
//        ^^^^^
// [diag.augmentationModifierExtra] The augmentation has the 'const' modifier that the declaration doesn't have.
}
