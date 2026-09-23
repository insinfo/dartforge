class A {
  A.named() {}
}
class B() extends A {
  this : super.named();
}
