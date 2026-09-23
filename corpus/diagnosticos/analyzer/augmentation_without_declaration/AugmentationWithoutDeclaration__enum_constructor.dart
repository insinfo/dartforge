enum A {
  v;
  const A();
}

augment enum A {;
  augment const A.named();
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
//                ^^^^^
// [diag.unusedElement] The declaration 'A.named' isn't referenced.
}
