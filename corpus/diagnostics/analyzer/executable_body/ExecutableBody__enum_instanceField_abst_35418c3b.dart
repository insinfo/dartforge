enum E {
  v;
  abstract final int foo;
//                   ^^^
// [diag.inducedGetterNotCompleteAfterAugmentations] The getter induced by 'foo' must have a body after all augmentations are applied.
  augment int get foo;
}
