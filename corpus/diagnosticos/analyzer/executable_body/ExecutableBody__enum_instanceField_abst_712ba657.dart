enum E {
  v;
  abstract int foo;
//             ^^^
// [diag.inducedGetterNotCompleteAfterAugmentations] The getter induced by 'foo' must have a body after all augmentations are applied.
  augment void set foo(int _) {}
}
