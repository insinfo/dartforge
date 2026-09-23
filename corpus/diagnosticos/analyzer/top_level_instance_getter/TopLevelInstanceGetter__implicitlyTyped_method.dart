class A {
  var x = 0;
  int f<T>(int x) => 0;
}
var a = new A();
var b = a.f(a.x);
