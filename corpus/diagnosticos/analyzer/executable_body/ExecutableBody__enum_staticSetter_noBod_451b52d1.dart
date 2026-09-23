enum E {
  v;
  static set foo(int _);
//           ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
  augment static set foo(int _);
}
