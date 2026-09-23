class A {
  var x = 0;
}
var a = new A();
var b = (<T>(y) => 0)(a.x);
