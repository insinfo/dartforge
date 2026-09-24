class A {
  var x = 0;
}
class B {
  B.named(x);
}
var a = new A();
var b = new B.named(a.x);
