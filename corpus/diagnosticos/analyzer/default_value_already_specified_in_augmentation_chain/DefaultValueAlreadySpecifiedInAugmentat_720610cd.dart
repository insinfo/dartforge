class A {
  A({int x = 0});
//       ^
// [context 1] The previous formal parameter with default value is here.

  augment A({int x = 0});
//                 ^
// [diag.defaultValueAlreadySpecifiedInAugmentationChain][context 1] The default value for this optional parameter was already specified in the augmentation chain.
}
