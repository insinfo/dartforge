class A {
  static int a = 0;
}
class B extends A {
  static int b = super.a;
//               ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
