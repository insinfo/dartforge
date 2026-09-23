extension type const E(int it);

augment extension type E {
  augment E(int it);
//^^^^^^^
// [diag.augmentationModifierMissing] The augmentation is missing the 'const' modifier that the declaration has.
}
