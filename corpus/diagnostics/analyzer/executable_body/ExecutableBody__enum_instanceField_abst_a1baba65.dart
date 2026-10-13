enum E {
  v;
  abstract int foo;
//             ^^^
// [diag.inducedSetterNotCompleteAfterAugmentations] The setter induced by 'foo' must have a body after all augmentations are applied.
  augment int get foo => 0;
  augment void set foo(int _);
}
