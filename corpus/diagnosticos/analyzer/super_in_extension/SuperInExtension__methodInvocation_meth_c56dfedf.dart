extension E on int {
  void foo() {
    super.foo();
//  ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
  }
}
