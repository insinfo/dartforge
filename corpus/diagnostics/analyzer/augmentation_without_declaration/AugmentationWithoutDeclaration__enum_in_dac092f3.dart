enum A {
  v;
  const A();
}

augment enum A {;
  augment int get foo => 0;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
