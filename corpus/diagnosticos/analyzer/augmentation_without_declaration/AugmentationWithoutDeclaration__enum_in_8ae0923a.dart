enum A {
  v;
  const A();
  static int get foo => 0;
}

augment enum A {;
  augment int get foo;
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
