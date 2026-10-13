class A {
  A._();
  factory A() => A._();
//        ^
// [context 1] The complete declaration is here.
  augment factory A() => A._();
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
