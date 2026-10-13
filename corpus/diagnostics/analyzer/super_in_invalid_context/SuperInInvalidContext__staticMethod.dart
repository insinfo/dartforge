class A {
  static m() {}
}
class B extends A {
  static n() { return super.m(); }
//                    ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
