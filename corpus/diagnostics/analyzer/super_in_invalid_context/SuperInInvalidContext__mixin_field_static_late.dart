class A {
  int get foo => 0;
}

mixin M on A {
  static late var f = super.foo;
//                    ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
