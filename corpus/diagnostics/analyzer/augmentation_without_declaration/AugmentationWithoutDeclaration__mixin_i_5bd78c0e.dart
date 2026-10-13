mixin A {}

augment mixin A {
  augment int foo = 0;
//            ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
