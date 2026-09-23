mixin M {
  int i = 0;
}
class A {
  const A();
}
class B = A with M;
var b = const B();
//      ^^^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
