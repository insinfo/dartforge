class A {
  var x = new B();
  int operator[](int value) => 0;
}
class B {
  int y = 0;
}
var a = new A();
var b = (a.x).y;
