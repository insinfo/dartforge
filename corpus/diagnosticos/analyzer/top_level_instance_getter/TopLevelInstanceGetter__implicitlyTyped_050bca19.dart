class A {
  var x = 0;
}
class B {
  B(x);
}
var a = new A();
var b = new B(a.x);
