enum E {
  v;

  factory E.named() => v;
}

augment enum E {
  ;
  augment E.named();
//^^^^^^^
// [diag.augmentationModifierMissing] The augmentation is missing the 'factory' modifier that the declaration has.
}
