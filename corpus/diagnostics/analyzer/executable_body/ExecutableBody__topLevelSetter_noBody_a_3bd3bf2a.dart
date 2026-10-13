set foo(int _);
//  ^^^
// [diag.functionNotCompleteAfterAugmentations] The function or member 'foo' must have a body after all augmentations are applied.
augment set foo(int _);
