class A {
  var x = 0;
  int f<T>(x) => 0;
}
var a = new A();
var b = a.f<int>(a.x);
