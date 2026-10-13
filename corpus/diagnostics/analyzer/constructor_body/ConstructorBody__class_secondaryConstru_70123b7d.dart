class A {
  final int x;
  A(this.x);
//^
// [context 1] The complete declaration is here.
  augment A(int x) {}
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
