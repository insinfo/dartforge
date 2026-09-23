class C {
  augment factory() => throw 0;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
