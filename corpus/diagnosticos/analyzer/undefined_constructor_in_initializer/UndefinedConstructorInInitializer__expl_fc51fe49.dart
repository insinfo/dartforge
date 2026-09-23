class A {
  A.named() {}
}
class B() extends A {
  this : super();
//       ^^^^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}
