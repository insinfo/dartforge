mixin A {}

augment mixin A {
  augment set foo(int _) {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
