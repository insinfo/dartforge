class A {
  A.named() {}
}
class B extends A {
  B() : super();
//      ^^^^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}
