class A {
  int get foo => 0;
}

extension E on int {
  static late var f = super.foo;
//                    ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
