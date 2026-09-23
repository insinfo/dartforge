enum E(int x) {
//   ^
// [context 1] The complete declaration is here.
  v(0);
  this : assert(true);
}
augment enum E {;
  augment E(int x) : assert(true);
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
}
