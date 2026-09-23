class A {
  m() {}
}
class B extends A {
  factory B() {
    super.m();
//  ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
    return B._();
  }
  B._();
}
