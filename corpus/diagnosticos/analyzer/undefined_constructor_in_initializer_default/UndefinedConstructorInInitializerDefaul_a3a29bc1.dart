class A {
  A.named();
}
class B extends A {
  new foo();
//^^^^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}
