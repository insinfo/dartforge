extension type E(int it);

augment extension type E {
  augment factory E(int it);
//        ^^^^^^^
// [diag.augmentationModifierExtra] The augmentation has the 'factory' modifier that the declaration doesn't have.
}
