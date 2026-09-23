class A {
  A() {}
//^
// [context 1] The complete declaration is here.
  augment A() {}
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
