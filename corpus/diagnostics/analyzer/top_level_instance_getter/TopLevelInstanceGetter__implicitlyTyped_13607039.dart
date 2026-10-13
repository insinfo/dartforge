class A {
  var x = 0;
  int operator[](int value) => 0;
}
var a = new A();
var b = a[a.x];
