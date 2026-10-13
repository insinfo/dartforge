enum A {
  v;
  const A();
  static void foo() {}
}

augment enum A {;
  augment void foo();
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
}
