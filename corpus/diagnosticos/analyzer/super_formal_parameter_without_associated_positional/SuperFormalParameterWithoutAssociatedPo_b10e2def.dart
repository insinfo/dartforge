class B extends A {
//              ^
// [diag.extendsNonClass] Classes can only extend other classes.
  B(super.x) : super.named();
//             ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'Object' doesn't have a constructor named 'named'.
}
