extension E on int {
  static void foo() {
    super.foo();
//  ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
  }
}
