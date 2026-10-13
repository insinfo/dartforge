class A {
  factory A.named();
//        ^^^^^^^
// [diag.factoryNotCompleteAfterAugmentations] The factory constructor 'named' must have a body or redirection after all augmentations are applied.
  augment factory A.named();
}
