class A {
  m() {}
}
class B extends A {
  var f;
  B() : f = super.m();
//          ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
