class A {
  var x = 0;
}
class B<T> {
  B(T x);
}
var a = new A();
var b = new B(a.x);
