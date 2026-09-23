class A {
  f() {}
}
class B {
  var v;
  B() : v = new A().f();
}
