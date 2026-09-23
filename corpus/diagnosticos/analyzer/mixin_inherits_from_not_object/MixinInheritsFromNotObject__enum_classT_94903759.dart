mixin class A {}
mixin class B = Object with A;
enum E with B {
  v
}
