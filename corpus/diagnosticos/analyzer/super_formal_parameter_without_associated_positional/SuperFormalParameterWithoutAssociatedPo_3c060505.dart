class A {
  A(int x);
}

class B extends A {
  B(super.x) : super.named();
//             ^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}
