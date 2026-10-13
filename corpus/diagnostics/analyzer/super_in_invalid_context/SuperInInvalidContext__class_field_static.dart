class A {
  int get foo => 0;
}

class B extends A {
  static var f = super.foo;
//               ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
