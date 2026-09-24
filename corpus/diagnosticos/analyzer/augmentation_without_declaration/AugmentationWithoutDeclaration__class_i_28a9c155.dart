class A {
  final int foo = 0;
//          ^^^
// [context 1] The corresponding getter is induced by this declaration.
}

augment class A {
  augment abstract int foo;
//                     ^^^
// [diag.augmentationWithoutSetterDeclaration][context 1] This augmentation induces a setter, but no setter declaration named 'foo' exists to augment.
}
