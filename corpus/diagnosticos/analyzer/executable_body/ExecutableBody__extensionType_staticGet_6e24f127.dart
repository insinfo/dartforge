extension type E(int i) {
  static int get foo;
//               ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
  augment static int get foo;
}
