enum E.named() {
  v.named();
}

augment enum E {
  ;
  augment factory E.named();
//^^^^^^^
// [diag.augmentationModifierMissing] The augmentation is missing the 'const' modifier that the declaration has.
//        ^^^^^^^
// [diag.augmentationModifierExtra] The augmentation has the 'factory' modifier that the declaration doesn't have.
}
