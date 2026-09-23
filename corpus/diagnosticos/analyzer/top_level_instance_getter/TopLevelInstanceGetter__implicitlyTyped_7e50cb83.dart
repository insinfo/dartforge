class A {
  var x = 0;
}
class B<T> {
  B.named(T x);
}
var a = new A();
var b = new B.named(a.x);
