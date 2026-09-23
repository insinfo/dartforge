class A {
  static abstract final int foo;
//                          ^^^
// [diag.inducedGetterNotCompleteAfterAugmentations] The getter induced by 'foo' must have a body after all augmentations are applied.
  augment static abstract final int foo;
}
