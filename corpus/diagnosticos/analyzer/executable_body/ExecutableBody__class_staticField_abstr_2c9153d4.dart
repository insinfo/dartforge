class A {
  static abstract int foo;
//                    ^^^
// [diag.inducedGetterNotCompleteAfterAugmentations] The getter induced by 'foo' must have a body after all augmentations are applied.
  augment static set foo(int _) {}
}
