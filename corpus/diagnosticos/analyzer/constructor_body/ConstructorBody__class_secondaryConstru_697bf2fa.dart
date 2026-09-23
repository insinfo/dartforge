class A {
  factory ();
//^^^^^^^
// [diag.factoryNotCompleteAfterAugmentations] The factory constructor 'new' must have a body or redirection after all augmentations are applied.
  augment factory ();
}
