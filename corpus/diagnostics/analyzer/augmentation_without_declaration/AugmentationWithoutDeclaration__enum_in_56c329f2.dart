enum A {
  v;
  const A();
  static int foo = 0;
}

augment enum A {;
  augment abstract int foo;
//                     ^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
