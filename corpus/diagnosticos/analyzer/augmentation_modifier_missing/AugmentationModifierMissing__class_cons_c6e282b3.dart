class A {
  A._();
  factory A() = A._;
  augment A();
//^^^^^^^
// [diag.augmentationModifierMissing] The augmentation is missing the 'factory' modifier that the declaration has.
}
