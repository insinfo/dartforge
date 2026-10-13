class A {
  const A();
  augment A();
//^^^^^^^
// [diag.augmentationModifierMissing] The augmentation is missing the 'const' modifier that the declaration has.
}
