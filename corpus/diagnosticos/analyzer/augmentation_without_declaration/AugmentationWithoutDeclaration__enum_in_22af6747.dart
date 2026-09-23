enum A {
  v;
  const A();
}

augment enum A {;
  augment set foo(int _) {}
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
