class A {
  A.named();
}
class B extends A {
  B.foo();
//^^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}
