extension E on int {
  int get foo;
//        ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
  augment int get foo;
}
