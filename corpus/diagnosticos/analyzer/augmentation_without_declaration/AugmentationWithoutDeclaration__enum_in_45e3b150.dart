enum A {
  v;
  const A();
}

augment enum A {;
  augment final int foo = 0;
//                  ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
