extension E on int {
  abstract int foo;
//             ^^^
// [diag.inducedGetterNotCompleteAfterAugmentations] The getter induced by 'foo' must have a body after all augmentations are applied.
  augment set foo(int _) {}
}
