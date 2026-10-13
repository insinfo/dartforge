class A {
  A.named() {}
}
class B {
  var v;
  B() : v = new A.named();
}
