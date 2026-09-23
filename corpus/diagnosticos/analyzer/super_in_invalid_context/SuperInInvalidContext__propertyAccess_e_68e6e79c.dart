class A {
  int get foo => 0;
}

extension E on int {
  static var f = super.foo;
//               ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
