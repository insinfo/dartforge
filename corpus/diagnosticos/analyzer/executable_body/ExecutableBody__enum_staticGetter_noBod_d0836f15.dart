enum E {
  v;
  static int get foo;
//               ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
  augment static int get foo;
}
