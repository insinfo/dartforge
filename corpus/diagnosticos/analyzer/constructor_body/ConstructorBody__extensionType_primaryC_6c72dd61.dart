extension type A(int it) {}
//             ^
// [context 1] The complete declaration is here.
augment extension type A {
  augment A(int it) {}
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
