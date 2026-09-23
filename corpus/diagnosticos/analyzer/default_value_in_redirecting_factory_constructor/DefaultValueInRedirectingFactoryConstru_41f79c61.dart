class A {
  A({int x = 0});
  factory A.foo({int x = 0}) = A;
//          ^^^
// [context 1] The complete declaration is here.
}

augment class A {
  augment factory A.foo({int x}) = A;
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
