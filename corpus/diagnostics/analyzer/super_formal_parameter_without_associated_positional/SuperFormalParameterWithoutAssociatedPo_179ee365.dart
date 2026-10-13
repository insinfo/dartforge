class A {
  A(int x);
}

class B(super.a) extends A {
  this : super.named();
//       ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}
