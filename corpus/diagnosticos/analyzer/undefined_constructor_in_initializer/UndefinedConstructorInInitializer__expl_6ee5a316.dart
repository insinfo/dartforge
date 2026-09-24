class A {}
class B() extends A {
  this : super.named();
//       ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}
