class A {}
class B extends A {}

dynamic f(bool c, B x, B y) {
  var r = c ? x as A : y;
  return r;
}
