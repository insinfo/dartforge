class A {
  static set foo(int _) {}
//           ^^^
// [context 1] The corresponding setter is declared here.
// [context 2] The complete declaration is here.
  augment static int foo = 1;
//                   ^^^
// [diag.augmentationWithoutGetterDeclaration][context 1] This augmentation induces a getter, but no getter declaration named 'foo' exists to augment.
// [diag.augmentationInducedSetterAlreadyComplete][context 2] The setter induced by this augmentation is complete, but the setter being augmented is already complete.
}
