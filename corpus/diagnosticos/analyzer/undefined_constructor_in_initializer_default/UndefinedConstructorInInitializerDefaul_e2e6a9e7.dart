class A {
  A.named();
}
class B.foo() extends A {
  this;
//^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}
