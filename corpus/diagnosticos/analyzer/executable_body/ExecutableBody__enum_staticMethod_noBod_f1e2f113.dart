enum E {
  v;
  static void foo();
//            ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
  augment static void foo();
}
