extension E on int {
  static final v = super.foo();
//                 ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
