class A {
  A(int x);
}
class B extends A {
  B(super.x);
//^
// [context 1] The complete declaration is here.
  augment B(int x) {}
//^^^^^^^
// [diag.constructorAlreadyComplete][context 1] The augmentation can't provide a body, initializers, or initializing formal or super formal parameters because the constructor is already complete.
//        ^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}
