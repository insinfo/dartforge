class A {
  var x = 0;
}
class B<T> {
  B.named(x);
}
var a = new A();
var b = new B<int>.named(a.x);
