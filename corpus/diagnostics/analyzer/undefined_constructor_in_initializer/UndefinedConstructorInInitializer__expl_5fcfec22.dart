class A {}
class B extends A {
  B() : super.named();
//      ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}
