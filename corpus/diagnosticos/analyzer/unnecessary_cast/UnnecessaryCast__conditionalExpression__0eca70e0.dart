class A {}
class B extends A {}

dynamic f(bool c, B x, B y) {
  return c ? x : y as A;
}
