class B(super.a) extends A {
//                       ^
// [diag.extendsNonClass] Classes can only extend other classes.
  this : super.named();
//       ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'Object' doesn't have a constructor named 'named'.
}
