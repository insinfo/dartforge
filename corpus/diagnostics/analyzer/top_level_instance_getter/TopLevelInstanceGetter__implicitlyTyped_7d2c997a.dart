class A {
  var x = 0;
}
class B<T> {
  B(x);
}
var a = new A();
var b = new B<int>(a.x);
