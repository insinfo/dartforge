extension E on int {
  int operator -(int _);
//             ^
// [diag.functionNotCompleteAfterAugmentations] The function or member '-' must have a body after all augmentations are applied.
  augment int operator -(int _);
}
